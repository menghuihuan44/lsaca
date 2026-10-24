#!/usr/bin/env python3
"""Cross-language benchmark harness for the lsaca compiler.

Measures, for the same four workloads implemented in lsaca, C, Rust, Python and
Node.js:

  * compile time  - lsaca cold build (fresh content-addressed cache, includes
                    CMake configure + C compiler), lsaca cached build, gcc -O2,
                    rustc -O; interpreters are reported as n/a;
  * runtime       - one warm-up run plus N measured runs; median and min;
  * peak memory   - peak working set of the child process (Windows: psapi
                    GetProcessMemoryInfo polling; POSIX: /usr/bin/time or
                    /proc/<pid>/status VmHWM).

Every measured run is validated against the checksum the program is supposed to
print; the harness exits non-zero on any mismatch.

Usage (from the repository root):

    python scripts/bench.py                    # full suite
    python scripts/bench.py --quick            # fewer repetitions
    python scripts/bench.py --build-mode fallback
    python scripts/bench.py --programs fib,loop_sum --languages c,rust

Raw data is written to benchmarks/results/latest.json, markdown tables to
benchmarks/results/latest.md and stdout.  The harness is stdlib-only,
cross-platform and non-interactive.

lsaca build notes: a normal release build goes through CMake (a content-
addressed cache directory is used, so a cold build means a fresh
LSACA_CACHE_DIR).  If CMake cannot be driven in the current environment the
harness falls back to "lsaca emit-c" plus a direct gcc -O2 -DNDEBUG compile of
the generated C, and labels those numbers accordingly.  --build-mode fallback
forces that path, --build-mode official disables the fallback.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

# ---------------------------------------------------------------------------
# Workloads
# ---------------------------------------------------------------------------

PROGRAMS = {
    "fib": {
        "expected": "checksum=8320400",
        "description": "recursive fibonacci(30), called 10 times, checksummed",
        "workload": {"n": 30, "repetitions": 10},
    },
    "loop_sum": {
        "expected": "checksum=32000004000000",
        "description": "integer sum 1..=8,000,000",
        "workload": {"n": 8000000},
    },
    "string_build": {
        "expected": "checksum=3890000,499500000",
        "description": "append 1,000,000 numeric tokens (i % 1000) to a string; "
                       "checksum is length,sum-of-token-values",
        "workload": {"tokens": 1000000, "delimiter": ","},
    },
    "vec_memory": {
        "expected": "checksum=1499998500000",
        "description": "build a vector of 1,000,000 Point{x=i64, y=i64}, then checksum",
        "workload": {"points": 1000000},
    },
}

LANGUAGES = ["lsaca", "c", "rust", "python", "node"]

SOURCE_EXT = {
    "lsaca": ".ls",
    "c": ".c",
    "rust": ".rs",
    "python": ".py",
    "node": ".js",
}

LANGUAGE_LABELS = {
    "lsaca": "lsaca",
    "c": "C (gcc -O2)",
    "rust": "Rust (rustc -O)",
    "python": "Python 3",
    "node": "Node.js",
}

# Languages that need a compiler toolchain; interpreters are reported as n/a.
COMPILED = ["lsaca", "c", "rust"]


# ---------------------------------------------------------------------------
# Small helpers
# ---------------------------------------------------------------------------


class HarnessError(RuntimeError):
    """Benchmark harness failure (bad toolchain, failed build, bad checksum)."""


def eprint(*args) -> None:
    print(*args, file=sys.stderr)


def now_iso() -> str:
    return datetime.now().astimezone().isoformat(timespec="seconds")


def rel_path(path, root: Path) -> str:
    try:
        return str(Path(path).resolve().relative_to(root))
    except ValueError:
        return str(path)


def sha256_file(path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def repository_commit(root: Path):
    """Best-effort short git commit for self-describing results (read-only)."""
    if not (root / ".git").exists():
        return None
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=str(root),
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    return (proc.stdout or "").strip() or None


def tail(text: str, limit: int = 1500) -> str:
    text = (text or "").strip()
    if len(text) <= limit:
        return text
    return "..." + text[-limit:]


def describe_failure(proc, limit: int = 1500) -> str:
    parts = []
    if proc.stdout and proc.stdout.strip():
        parts.append("stdout:\n" + tail(proc.stdout, limit))
    if proc.stderr and proc.stderr.strip():
        parts.append("stderr:\n" + tail(proc.stderr, limit))
    if not parts:
        return "(no output)"
    return "\n".join(parts)


def timed_run(cmd, env, cwd=None, timeout=600.0):
    """Run a command, return (CompletedProcess, wall_seconds)."""
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            [str(part) for part in cmd],
            cwd=str(cwd) if cwd is not None else None,
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise HarnessError(
            "command timed out after %.0f s: %s" % (timeout, " ".join(str(p) for p in cmd))
        )
    except OSError as exc:
        raise HarnessError("could not run %s: %s" % (" ".join(str(p) for p in cmd), exc))
    return proc, time.perf_counter() - start


def capture_first_line(cmd, env, timeout=30.0):
    try:
        proc = subprocess.run(
            [str(part) for part in cmd],
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    text = (proc.stdout or "").strip() or (proc.stderr or "").strip()
    if not text:
        return None
    return text.splitlines()[0].strip()


# ---------------------------------------------------------------------------
# Environment / tool discovery
# ---------------------------------------------------------------------------


def known_tool_dirs():
    """Well-known install locations that are often missing from PATH."""
    if os.name != "nt":
        return []
    home = Path.home()
    return [
        Path(r"C:\msys64\ucrt64\bin"),
        Path(r"C:\msys64\mingw64\bin"),
        Path(r"C:\msys64\mingw32\bin"),
        Path(r"C:\Program Files\CMake\bin"),
        home / ".cargo" / "bin",
    ]


def build_child_env(root: Path):
    """Environment for every child process.

    - prepend well-known tool directories (MSYS2 gcc, CMake, cargo) to PATH so
      a plain "python scripts/bench.py" invocation works without a developer
      prompt;
    - point TEMP/TMP/TMPDIR inside the workspace: the DSH sandbox on this host
      blocks builds whose temp files land outside the workspace, and the lsaca
      driver also redirects the CMake/gcc environment itself;
    - drop LSACA_* overrides so measurements are deterministic.
    """
    env = dict(os.environ)
    seen = set()
    path_parts = []
    prepended = []
    for directory in known_tool_dirs():
        if directory.is_dir():
            key = str(directory).lower()
            if key not in seen:
                seen.add(key)
                path_parts.append(str(directory))
                prepended.append(str(directory))
    for part in env.get("PATH", "").split(os.pathsep):
        if part:
            key = part.lower()
            if key not in seen:
                seen.add(key)
                path_parts.append(part)
    env["PATH"] = os.pathsep.join(path_parts)
    tmp_dir = root / "target" / "bench-tmp" / "tmp"
    tmp_dir.mkdir(parents=True, exist_ok=True)
    for var in ("TEMP", "TMP", "TMPDIR"):
        env[var] = str(tmp_dir)
    for var in ("LSACA_CACHE_DIR", "LSACA_CC", "LSACA_CFLAGS"):
        env.pop(var, None)
    env["PYTHONIOENCODING"] = "utf-8"
    return env, prepended, tmp_dir


def find_tool(name, env, extra_candidates=()):
    found = shutil.which(name, path=env.get("PATH", ""))
    if found:
        return found
    for candidate in extra_candidates:
        path = Path(candidate)
        if path.is_file():
            return str(path)
    return None


def detect_tools(env):
    home = Path.home()
    return {
        "gcc": find_tool(
            "gcc",
            env,
            [
                r"C:\msys64\ucrt64\bin\gcc.exe",
                r"C:\msys64\mingw64\bin\gcc.exe",
                r"C:\msys64\clang64\bin\gcc.exe",
            ],
        ),
        "rustc": find_tool("rustc", env, [home / ".cargo" / "bin" / "rustc.exe"]),
        "node": find_tool("node", env),
        "cmake": find_tool("cmake", env, [r"C:\Program Files\CMake\bin\cmake.exe"]),
        "ninja": find_tool("ninja", env, [r"C:\msys64\ucrt64\bin\ninja.exe"]),
        "python": sys.executable,
    }


def machine_info():
    info = {
        "platform": platform.platform(),
        "system": platform.system(),
        "release": platform.release(),
        "version": platform.version(),
        "machine": platform.machine(),
        "logical_cpus": os.cpu_count(),
        "cpu": platform.processor() or None,
        "ram_gib": None,
    }
    system = platform.system()
    if system == "Windows":
        try:
            import winreg

            key_path = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"
            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, key_path) as key:
                info["cpu"] = str(winreg.QueryValueEx(key, "ProcessorNameString")[0]).strip()
        except OSError:
            pass
        try:
            import ctypes

            class MEMORYSTATUSEX(ctypes.Structure):
                _fields_ = [
                    ("dwLength", ctypes.c_ulong),
                    ("dwMemoryLoad", ctypes.c_ulong),
                    ("ullTotalPhys", ctypes.c_ulonglong),
                    ("ullAvailPhys", ctypes.c_ulonglong),
                    ("ullTotalPageFile", ctypes.c_ulonglong),
                    ("ullAvailPageFile", ctypes.c_ulonglong),
                    ("ullTotalVirtual", ctypes.c_ulonglong),
                    ("ullAvailVirtual", ctypes.c_ulonglong),
                    ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
                ]

            status = MEMORYSTATUSEX()
            status.dwLength = ctypes.sizeof(MEMORYSTATUSEX)
            if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
                info["ram_gib"] = round(status.ullTotalPhys / (1024**3), 2)
        except OSError:
            pass
    elif system == "Linux":
        try:
            for line in Path("/proc/cpuinfo").read_text(encoding="utf-8", errors="replace").splitlines():
                if line.lower().startswith("model name"):
                    info["cpu"] = line.split(":", 1)[1].strip()
                    break
        except OSError:
            pass
        try:
            for line in Path("/proc/meminfo").read_text(encoding="utf-8", errors="replace").splitlines():
                if line.startswith("MemTotal:"):
                    info["ram_gib"] = round(int(line.split()[1]) / (1024**2), 2)
                    break
        except (OSError, ValueError, IndexError):
            pass
    elif system == "Darwin":
        try:
            out = subprocess.run(
                ["sysctl", "-n", "machdep.cpu.brand_string"],
                capture_output=True,
                text=True,
                timeout=10,
            ).stdout.strip()
            if out:
                info["cpu"] = out
        except (OSError, subprocess.TimeoutExpired):
            pass
        try:
            out = subprocess.run(
                ["sysctl", "-n", "hw.memsize"], capture_output=True, text=True, timeout=10
            ).stdout.strip()
            if out:
                info["ram_gib"] = round(int(out) / (1024**3), 2)
        except (OSError, subprocess.TimeoutExpired, ValueError):
            pass
    if info["ram_gib"] is None:
        try:
            info["ram_gib"] = round(
                os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") / (1024**3), 2
            )
        except (ValueError, OSError, AttributeError):
            pass
    return info


# ---------------------------------------------------------------------------
# Peak memory measurement
# ---------------------------------------------------------------------------

PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000


def peak_memory_run(cmd, env, cwd):
    if os.name == "nt":
        return _windows_peak_memory(cmd, env, cwd)
    return _posix_peak_memory(cmd, env, cwd)


def _windows_peak_memory(cmd, env, cwd):
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)

    class PROCESS_MEMORY_COUNTERS(ctypes.Structure):
        _fields_ = [
            ("cb", wintypes.DWORD),
            ("PageFaultCount", wintypes.DWORD),
            ("PeakWorkingSetSize", ctypes.c_size_t),
            ("WorkingSetSize", ctypes.c_size_t),
            ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
            ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
            ("PagefileUsage", ctypes.c_size_t),
            ("PeakPagefileUsage", ctypes.c_size_t),
        ]

    psapi.GetProcessMemoryInfo.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(PROCESS_MEMORY_COUNTERS),
        wintypes.DWORD,
    ]
    psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
    if hasattr(kernel32, "K32GetProcessMemoryInfo"):
        kernel32.K32GetProcessMemoryInfo.argtypes = psapi.GetProcessMemoryInfo.argtypes
        kernel32.K32GetProcessMemoryInfo.restype = wintypes.BOOL
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]

    def peak_of(handle):
        if not handle:
            return 0
        counters = PROCESS_MEMORY_COUNTERS()
        counters.cb = ctypes.sizeof(PROCESS_MEMORY_COUNTERS)
        ok = psapi.GetProcessMemoryInfo(
            wintypes.HANDLE(handle), ctypes.byref(counters), counters.cb
        )
        if not ok:
            try:
                ok = kernel32.K32GetProcessMemoryInfo(
                    wintypes.HANDLE(handle), ctypes.byref(counters), counters.cb
                )
            except AttributeError:
                ok = 0
        return int(counters.PeakWorkingSetSize) if ok else 0

    proc = subprocess.Popen(
        [str(part) for part in cmd],
        cwd=str(cwd) if cwd is not None else None,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    handle = None
    deadline = time.perf_counter() + 0.5
    while time.perf_counter() < deadline:
        handle = kernel32.OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, proc.pid
        )
        if not handle:
            handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, proc.pid)
        if handle:
            break
        if proc.poll() is not None:
            break
        time.sleep(0.001)
    peak = peak_of(handle)
    while proc.poll() is None:
        peak = max(peak, peak_of(handle))
        time.sleep(0.005)
    peak = max(peak, peak_of(handle))
    stdout, stderr = proc.communicate()
    if handle:
        kernel32.CloseHandle(handle)
    if peak > 0:
        return {
            "peak_bytes": peak,
            "method": "psapi GetProcessMemoryInfo PeakWorkingSetSize (polling)",
            "stdout": stdout,
            "stderr": stderr,
            "returncode": proc.returncode,
        }
    fallback = _powershell_peak_memory(cmd, env, cwd)
    fallback.setdefault("method", "PowerShell Process.PeakWorkingSet64 (polling)")
    return fallback


def _ps_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def _powershell_peak_memory(cmd, env, cwd):
    tmp_root = Path(env.get("TEMP") or (Path.cwd() / "target" / "bench-tmp" / "tmp"))
    tmp_root.mkdir(parents=True, exist_ok=True)
    token = "%d-%d" % (os.getpid(), time.time_ns())
    out_file = tmp_root / ("ps-mem-out-%s.txt" % token)
    err_file = tmp_root / ("ps-mem-err-%s.txt" % token)
    arguments = " ".join(_ps_quote(str(part)) for part in cmd[1:])
    script = r"""
$ErrorActionPreference = 'Stop'
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = %s
$psi.Arguments = %s
$psi.WorkingDirectory = %s
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$p = New-Object System.Diagnostics.Process
$p.StartInfo = $psi
$null = $p.Start()
$peak = 0
while (-not $p.HasExited) {
    try { $p.Refresh(); if ($p.PeakWorkingSet64 -gt $peak) { $peak = $p.PeakWorkingSet64 } } catch { }
    Start-Sleep -Milliseconds 5
}
$p.WaitForExit()
$out = $p.StandardOutput.ReadToEnd()
$err = $p.StandardError.ReadToEnd()
[System.IO.File]::WriteAllText(%s, $out)
[System.IO.File]::WriteAllText(%s, $err)
Write-Output ("PEAK=" + $peak)
Write-Output ("EXIT=" + $p.ExitCode)
""" % (
        _ps_quote(str(cmd[0])),
        _ps_quote(arguments),
        _ps_quote(str(cwd) if cwd is not None else os.getcwd()),
        _ps_quote(str(out_file)),
        _ps_quote(str(err_file)),
    )
    try:
        proc = subprocess.run(
            ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=300,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {
            "peak_bytes": None,
            "method": "unavailable",
            "note": "psapi and PowerShell memory sampling both failed: %s" % exc,
            "stdout": "",
            "stderr": "",
            "returncode": None,
        }
    peak = 0
    exit_code = None
    for line in (proc.stdout or "").splitlines():
        if line.startswith("PEAK="):
            try:
                peak = int(line[len("PEAK="):].strip())
            except ValueError:
                peak = 0
        elif line.startswith("EXIT="):
            try:
                exit_code = int(line[len("EXIT="):].strip())
            except ValueError:
                exit_code = None
    stdout = out_file.read_text(encoding="utf-8", errors="replace") if out_file.is_file() else ""
    stderr = err_file.read_text(encoding="utf-8", errors="replace") if err_file.is_file() else ""
    for path in (out_file, err_file):
        try:
            path.unlink()
        except OSError:
            pass
    return {
        "peak_bytes": peak or None,
        "method": "PowerShell Process.PeakWorkingSet64 (polling)",
        "stdout": stdout,
        "stderr": stderr,
        "returncode": exit_code,
    }


def _posix_peak_memory(cmd, env, cwd):
    # Preferred: /usr/bin/time, which reports the true peak RSS in one run.
    time_bin = Path("/usr/bin/time")
    if time_bin.is_file():
        if platform.system() == "Linux":
            argv = [str(time_bin), "-v"] + [str(part) for part in cmd]
            pattern = re.compile(r"^\s*Maximum resident set size \(kbytes\):\s*(\d+)", re.M)
            multiplier = 1024
            method = "/usr/bin/time -v (Maximum resident set size)"
        else:
            argv = [str(time_bin), "-l"] + [str(part) for part in cmd]
            pattern = re.compile(r"^\s*(\d+)\s+maximum resident set size", re.M)
            multiplier = 1
            method = "/usr/bin/time -l (maximum resident set size)"
        try:
            proc = subprocess.run(
                argv,
                cwd=str(cwd) if cwd is not None else None,
                env=env,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=300,
            )
        except (OSError, subprocess.TimeoutExpired):
            proc = None
        if proc is not None:
            match = pattern.search(proc.stderr or "")
            if match:
                return {
                    "peak_bytes": int(match.group(1)) * multiplier,
                    "method": method,
                    "stdout": proc.stdout,
                    "stderr": proc.stderr,
                    "returncode": proc.returncode,
                }
    # Fallback: poll VmHWM from /proc (Linux only).
    if Path("/proc").is_dir():
        proc = subprocess.Popen(
            [str(part) for part in cmd],
            cwd=str(cwd) if cwd is not None else None,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        status_path = Path("/proc") / str(proc.pid) / "status"
        peak_kib = 0
        vhwm = re.compile(r"^VmHWM:\s*(\d+)\s*kB", re.M)
        while proc.poll() is None:
            try:
                match = vhwm.search(status_path.read_text(encoding="utf-8", errors="replace"))
                if match:
                    peak_kib = max(peak_kib, int(match.group(1)))
            except OSError:
                pass
            time.sleep(0.002)
        stdout, stderr = proc.communicate()
        if peak_kib:
            return {
                "peak_bytes": peak_kib * 1024,
                "method": "/proc/<pid>/status VmHWM (polling)",
                "stdout": stdout,
                "stderr": stderr,
                "returncode": proc.returncode,
            }
        return {
            "peak_bytes": None,
            "method": "unavailable",
            "note": "could not sample VmHWM (process too short-lived?)",
            "stdout": stdout,
            "stderr": stderr,
            "returncode": proc.returncode,
        }
    return {
        "peak_bytes": None,
        "method": "unavailable",
        "note": "no /usr/bin/time and no /proc on this platform",
        "stdout": "",
        "stderr": "",
        "returncode": None,
    }



# ---------------------------------------------------------------------------
# Harness
# ---------------------------------------------------------------------------


class Harness:
    def __init__(self, args):
        self.args = args
        self.root = Path(args.root).resolve()
        self.programs_dir = self.root / "benchmarks" / "programs"
        if not self.programs_dir.is_dir():
            raise HarnessError(
                "benchmark programs not found at %s (run from the repository root)"
                % self.programs_dir
            )
        self.results_dir = self.root / "benchmarks" / "results"
        self.results_dir.mkdir(parents=True, exist_ok=True)
        self.tmp_root = self.root / "target" / "bench-tmp"
        self.tmp_root.mkdir(parents=True, exist_ok=True)
        self.env, self.path_prepend, self.tmp_dir = build_child_env(self.root)
        self.tools = detect_tools(self.env)
        self.warnings = []
        self.lsaca_mode = None  # None (not tried yet) / "official" / "fallback"
        self.official_attempt_error = None
        self.compiler = None
        self.lsaca_version = None
        self.programs = (
            [p for p in PROGRAMS if p in args.programs] if args.programs else list(PROGRAMS)
        )
        self.languages = (
            [l for l in LANGUAGES if l in args.languages] if args.languages else list(LANGUAGES)
        )
        # Fresh scratch area for this run (build outputs, caches).
        shutil.rmtree(self.tmp_root / "build", ignore_errors=True)
        shutil.rmtree(self.tmp_root / "cache", ignore_errors=True)

    # -- setup ------------------------------------------------------------

    def ensure_compiler(self):
        target_dir = self.root / "target" / "bench-tools"
        target_dir.mkdir(parents=True, exist_ok=True)
        dest = target_dir / ("lsaca.exe" if os.name == "nt" else "lsaca")
        if self.args.compiler:
            src = Path(self.args.compiler).resolve()
        else:
            src = self.root / "target" / "release" / ("lsaca.exe" if os.name == "nt" else "lsaca")
        if not src.is_file():
            if dest.is_file():
                self.warnings.append(
                    "%s is missing; reusing the previously copied compiler at %s"
                    % (rel_path(src, self.root), rel_path(dest, self.root))
                )
                src = dest
            else:
                raise HarnessError(
                    "lsaca compiler not found. Build it first: cargo build --release -p lsaca-cli. "
                    "Expected target/release/lsaca.exe"
                )
        if src.resolve() != dest.resolve():
            shutil.copy2(src, dest)
        self.compiler = dest
        version = capture_first_line([dest, "version"], self.env)
        if not version or "lsaca" not in version.lower():
            raise HarnessError(
                "the copied compiler at %s did not answer 'version' correctly (got %r)"
                % (rel_path(dest, self.root), version)
            )
        self.lsaca_version = version

    def toolchain_info(self):
        info = {
            "lsaca": self.lsaca_version,
            "lsaca_compiler": rel_path(self.compiler, self.root) if self.compiler else None,
            "python": "Python " + platform.python_version(),
        }
        if self.compiler is not None:
            info["lsaca_compiler_sha256"] = sha256_file(self.compiler)[:16]
            info["lsaca_compiler_mtime"] = (
                datetime.fromtimestamp(self.compiler.stat().st_mtime)
                .astimezone()
                .isoformat(timespec="seconds")
            )
        commit = repository_commit(self.root)
        if commit:
            info["repository_commit"] = commit
        if self.tools["gcc"]:
            info["gcc"] = capture_first_line([self.tools["gcc"], "--version"], self.env)
        if self.tools["rustc"]:
            info["rustc"] = capture_first_line([self.tools["rustc"], "--version"], self.env)
        if self.tools["node"]:
            info["node"] = capture_first_line([self.tools["node"], "--version"], self.env)
        if self.tools["cmake"]:
            info["cmake"] = capture_first_line([self.tools["cmake"], "--version"], self.env)
        if self.tools["ninja"]:
            info["ninja"] = capture_first_line([self.tools["ninja"], "--version"], self.env)
        return info

    def available(self, language):
        if language == "lsaca":
            return self.compiler is not None
        if language == "c":
            return bool(self.tools["gcc"])
        if language == "rust":
            return bool(self.tools["rustc"])
        if language == "python":
            return bool(self.tools["python"])
        if language == "node":
            return bool(self.tools["node"])
        return False

    def unavailable_reason(self, language):
        if language == "lsaca":
            return "lsaca compiler not available"
        if language == "c":
            return "gcc not found on PATH"
        if language == "rust":
            return "rustc not found on PATH (cargo bin is normally added automatically)"
        if language == "node":
            return "node not found on PATH"
        return "tool not found"

    # -- paths ------------------------------------------------------------

    def src_path(self, program, language):
        return self.programs_dir / (program + SOURCE_EXT[language])

    def fresh_cache_dir(self, label):
        path = self.tmp_root / "cache" / ("%s-%d-%d" % (label, os.getpid(), time.time_ns()))
        path.mkdir(parents=True, exist_ok=False)
        return path

    def build_dir(self, program, language):
        path = self.tmp_root / "build" / program / language
        path.mkdir(parents=True, exist_ok=True)
        return path

    def find_built_exe(self, outdir, program):
        expected = program + (".exe" if os.name == "nt" else "")
        candidate = outdir / expected
        if candidate.is_file():
            return candidate
        for path in sorted(outdir.iterdir()):
            if path.is_file() and path.suffix.lower() == ".exe":
                return path
            if (
                os.name != "nt"
                and path.is_file()
                and not path.suffix
                and os.access(path, os.X_OK)
            ):
                return path
        return None

    # -- builds -----------------------------------------------------------

    def build_lsaca(self, program, src, outdir):
        if self.args.build_mode == "fallback":
            record = self.build_lsaca_fallback(program, src, outdir)
            record["mode"] = "fallback"
            self.lsaca_mode = "fallback"
            return record
        if self.lsaca_mode != "fallback":
            try:
                record = self.build_lsaca_official(program, src, outdir)
                record["mode"] = "official"
                self.lsaca_mode = "official"
                return record
            except HarnessError as exc:
                self.official_attempt_error = str(exc)
                if self.args.build_mode == "official":
                    raise
                self.lsaca_mode = "fallback"
                first_line = str(exc).splitlines()[0]
                self.warnings.append(
                    "official lsaca build (CMake) failed or timed out; falling back to "
                    "lsaca emit-c + gcc for every program. Reason: " + first_line
                )
                eprint("[bench] NOTE: official lsaca build failed, using emit-c + gcc fallback")
                eprint("[bench]       " + first_line)
                record = self.build_lsaca_fallback(program, src, outdir)
                record["mode"] = "fallback"
                record["official_attempt_error"] = tail(str(exc), 2000)
                return record
        record = self.build_lsaca_fallback(program, src, outdir)
        record["mode"] = "fallback"
        return record

    def build_lsaca_official(self, program, src, outdir):
        cache = self.fresh_cache_dir(program + "-official-cold")
        env = dict(self.env)
        env["LSACA_CACHE_DIR"] = str(cache)
        outdir.mkdir(parents=True, exist_ok=True)
        cmd = [self.compiler, "build", "-r", str(src), "-o", str(outdir)]
        proc, cold_s = timed_run(cmd, env, cwd=self.root, timeout=self.args.official_timeout)
        if proc.returncode != 0:
            raise HarnessError(
                "official lsaca build failed for %s (exit %s):\n%s"
                % (program, proc.returncode, describe_failure(proc))
            )
        exe = self.find_built_exe(outdir, program)
        if exe is None:
            raise HarnessError(
                "official lsaca build reported success but no executable was found in %s"
                % rel_path(outdir, self.root)
            )
        cached = []
        for _ in range(self.compile_repeats):
            proc_cached, cached_s = timed_run(
                cmd, env, cwd=self.root, timeout=self.args.official_timeout
            )
            if proc_cached.returncode != 0:
                raise HarnessError(
                    "cached lsaca build failed for %s (exit %s):\n%s"
                    % (program, proc_cached.returncode, describe_failure(proc_cached))
                )
            cached.append(round(cached_s, 6))
        return {
            "kind": "lsaca-official",
            "cold_s": round(cold_s, 6),
            "cached_runs_s": cached,
            "cached_median_s": round(statistics.median(cached), 6) if cached else None,
            "exe": rel_path(exe, self.root),
            "note": "cold build uses a fresh LSACA_CACHE_DIR (CMake configure + C compiler); "
                    "cached build re-uses the content-addressed cache directory",
        }

    def build_lsaca_fallback(self, program, src, outdir):
        gen = outdir / "generated"
        outdir.mkdir(parents=True, exist_ok=True)
        env = dict(self.env)
        env["LSACA_CACHE_DIR"] = str(self.fresh_cache_dir(program + "-emit"))
        emit_runs = []
        for _ in range(self.compile_repeats):
            shutil.rmtree(gen, ignore_errors=True)
            proc, emit_s = timed_run(
                [self.compiler, "emit-c", str(src), "-o", str(gen)],
                env,
                cwd=self.root,
                timeout=120,
            )
            if proc.returncode != 0:
                raise HarnessError(
                    "lsaca emit-c failed for %s:\n%s" % (program, describe_failure(proc))
                )
            emit_runs.append(round(emit_s, 6))
        for name in ("main.c", "lsaca_rt.h", "lsaca_rt.c"):
            if not (gen / name).is_file():
                raise HarnessError(
                    "lsaca emit-c did not produce %s in %s" % (name, rel_path(gen, self.root))
                )
        exe = outdir / ("lsaca_" + program + (".exe" if os.name == "nt" else ""))
        if not self.tools["gcc"]:
            raise HarnessError("gcc is required for the lsaca fallback build path")
        gcc_runs = []
        for _ in range(self.compile_repeats):
            if exe.exists():
                exe.unlink()
            cmd = [
                self.tools["gcc"],
                "-O2",
                "-DNDEBUG",
                str(gen / "main.c"),
                str(gen / "lsaca_rt.c"),
                "-o",
                str(exe),
            ]
            proc, gcc_s = timed_run(cmd, env, cwd=gen, timeout=300)
            if proc.returncode != 0:
                raise HarnessError(
                    "gcc failed to compile the C generated by lsaca for %s:\n%s"
                    % (program, describe_failure(proc))
                )
            gcc_runs.append(round(gcc_s, 6))
        return {
            "kind": "lsaca-fallback",
            "emit_c_runs_s": emit_runs,
            "gcc_runs_s": gcc_runs,
            "emit_c_median_s": round(statistics.median(emit_runs), 6),
            "gcc_median_s": round(statistics.median(gcc_runs), 6),
            "total_s": round(statistics.median(emit_runs) + statistics.median(gcc_runs), 6),
            "exe": rel_path(exe, self.root),
            "note": "fallback: lsaca emit-c plus a direct gcc -O2 -DNDEBUG compile of the "
                    "generated C (the official CMake build could not be driven in this environment)",
        }

    def build_native(self, language, program, src, outdir):
        exe = outdir / (program + "_" + language + (".exe" if os.name == "nt" else ""))
        if language == "c":
            cmd = [self.tools["gcc"], "-O2", str(src), "-o", str(exe)]
        elif language == "rust":
            cmd = [self.tools["rustc"], "-O", str(src), "-o", str(exe)]
        else:
            raise HarnessError("build_native called for %r" % language)
        runs = []
        for _ in range(self.compile_repeats):
            if exe.exists():
                exe.unlink()
            proc, compile_s = timed_run(cmd, self.env, cwd=outdir, timeout=300)
            if proc.returncode != 0:
                raise HarnessError(
                    "%s compile failed for %s:\n%s"
                    % (language, program, describe_failure(proc))
                )
            runs.append(round(compile_s, 6))
        return {
            "kind": "native",
            "runs_s": runs,
            "median_s": round(statistics.median(runs), 6),
            "min_s": round(min(runs), 6),
            "exe": rel_path(exe, self.root),
            "command": " ".join(str(part) for part in cmd),
        }

    # -- execution --------------------------------------------------------

    def program_command(self, language, program, exe_rel):
        if language == "python":
            return [self.tools["python"], str(self.src_path(program, language))]
        if language == "node":
            return [self.tools["node"], str(self.src_path(program, language))]
        return [str(self.root / exe_rel)]

    def validate_output(self, proc, expected, language, program, phase):
        if proc.returncode != 0:
            raise HarnessError(
                "%s/%s %s run failed (exit %s):\n%s"
                % (program, language, phase, proc.returncode, describe_failure(proc))
            )
        got = (proc.stdout or "").strip()
        if got != expected:
            raise HarnessError(
                "%s/%s %s checksum mismatch: expected %r, got %r\n%s"
                % (program, language, phase, expected, got, describe_failure(proc, 800))
            )

    def measure_runtime(self, language, program, cmd, expected):
        for _ in range(self.args.warmups):
            proc, _ = timed_run(cmd, self.env, cwd=self.root, timeout=self.args.run_timeout)
            self.validate_output(proc, expected, language, program, "warm-up")
        times = []
        for _ in range(self.args.runs):
            proc, elapsed = timed_run(cmd, self.env, cwd=self.root, timeout=self.args.run_timeout)
            self.validate_output(proc, expected, language, program, "runtime")
            times.append(elapsed)
        return {
            "runs_s": [round(value, 6) for value in times],
            "median_s": round(statistics.median(times), 6),
            "min_s": round(min(times), 6),
            "max_s": round(max(times), 6),
            "warmups": self.args.warmups,
        }

    def measure_memory(self, language, program, cmd, expected):
        result = peak_memory_run(cmd, self.env, self.root)
        stdout = result.get("stdout") or ""
        returncode = result.get("returncode")
        fake = subprocess.CompletedProcess(
            cmd, returncode if returncode is not None else 0, stdout, result.get("stderr") or ""
        )
        self.validate_output(fake, expected, language, program, "memory")
        peak = result.get("peak_bytes")
        return {
            "peak_mib": round(peak / (1024.0 * 1024.0), 2) if peak else None,
            "method": result.get("method"),
            "note": result.get("note"),
        }

    # -- main loop --------------------------------------------------------

    def run(self):
        results = {
            "schema_version": 1,
            "generated_at": now_iso(),
            "harness": "scripts/bench.py",
            "host": machine_info(),
            "toolchains": self.toolchain_info(),
            "settings": {
                "runs": self.args.runs,
                "warmups": self.args.warmups,
                "compile_repeats": self.compile_repeats,
                "quick": self.args.quick,
                "build_mode": self.args.build_mode,
                "official_timeout_s": self.args.official_timeout,
                "memory": not self.args.no_memory,
                "programs": self.programs,
                "languages": self.languages,
                "child_temp_dir": rel_path(self.tmp_dir, self.root),
                "path_prepended": self.path_prepend,
            },
            "lsaca_build": {
                "mode": None,
                "compiler": rel_path(self.compiler, self.root) if self.compiler else None,
                "official_attempt_error": None,
            },
            "programs": {},
            "warnings": self.warnings,
        }
        total = len(self.programs) * len(self.languages)
        done = 0
        for program in self.programs:
            spec = PROGRAMS[program]
            entry = {
                "expected": spec["expected"],
                "description": spec["description"],
                "workload": spec["workload"],
                "sources": {
                    language: rel_path(self.src_path(program, language), self.root)
                    for language in self.languages
                },
                "languages": {},
            }
            for language in self.languages:
                done += 1
                if not self.available(language):
                    reason = self.unavailable_reason(language)
                    self.warnings.append("%s/%s skipped: %s" % (program, language, reason))
                    entry["languages"][language] = {"skipped": reason}
                    continue
                src = self.src_path(program, language)
                if not src.is_file():
                    raise HarnessError(
                        "missing source file for %s/%s: %s"
                        % (program, language, rel_path(src, self.root))
                    )
                outdir = self.build_dir(program, language)
                if language == "lsaca":
                    compile_record = self.build_lsaca(program, src, outdir)
                    command = self.program_command(language, program, compile_record["exe"])
                elif language in ("c", "rust"):
                    compile_record = self.build_native(language, program, src, outdir)
                    command = self.program_command(language, program, compile_record["exe"])
                else:
                    compile_record = {
                        "kind": "interpreted",
                        "exe": None,
                        "note": "interpreted language: no ahead-of-time compile step",
                    }
                    command = self.program_command(language, program, None)
                runtime = self.measure_runtime(language, program, command, spec["expected"])
                memory = None
                if not self.args.no_memory:
                    memory = self.measure_memory(language, program, command, spec["expected"])
                entry["languages"][language] = {
                    "compile": compile_record,
                    "runtime": runtime,
                    "memory": memory,
                }
                eprint(
                    "[%d/%d] %-12s %-6s  compile=%-26s median=%9.2f ms  peak=%s"
                    % (
                        done,
                        total,
                        program,
                        language,
                        _compile_summary(compile_record),
                        runtime["median_s"] * 1000.0,
                        ("%.1f MiB" % memory["peak_mib"])
                        if memory and memory["peak_mib"]
                        else "n/a",
                    )
                )
            results["programs"][program] = entry
        results["lsaca_build"]["mode"] = self.lsaca_mode
        results["lsaca_build"]["official_attempt_error"] = (
            tail(self.official_attempt_error, 2000) if self.official_attempt_error else None
        )
        results["warnings"] = self.warnings
        results["validation"] = "all measured runs matched the expected checksum"
        return results


def _compile_summary(record):
    if record is None:
        return "n/a"
    kind = record.get("kind")
    if kind == "lsaca-official":
        return "official cold %.3f s" % record["cold_s"]
    if kind == "lsaca-fallback":
        return "emit-c+gcc %.3f s" % record["total_s"]
    if kind == "native":
        return "%.3f s" % record["median_s"]
    return "n/a (interpreted)"


# ---------------------------------------------------------------------------
# Markdown rendering
# ---------------------------------------------------------------------------


def fmt_seconds(value):
    if value is None:
        return "n/a"
    if value >= 10.0:
        return "%.2f" % value
    return "%.3f" % value


def fmt_runtime(record):
    if not record or record.get("median_s") is None:
        return "n/a"
    return "%.1f (%.1f)" % (record["median_s"] * 1000.0, record["min_s"] * 1000.0)


def fmt_memory(record):
    if not record or record.get("peak_mib") is None:
        return "n/a"
    return "%.1f" % record["peak_mib"]


def render_markdown(results):
    host = results["host"]
    tools = results["toolchains"]
    programs = list(results["programs"])
    languages = [l for l in LANGUAGES if l in results["settings"]["languages"]]
    lines = []
    lines.append("# lsaca cross-language benchmarks")
    lines.append("")
    lines.append("Generated: %s  " % results["generated_at"])
    lines.append("Harness: python scripts/bench.py (see benchmarks/README.md)")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    lines.append("- OS: %s" % host["platform"])
    lines.append(
        "- CPU: %s (%s logical CPUs)"
        % (host.get("cpu") or "unknown", host.get("logical_cpus"))
    )
    lines.append("- RAM: %s GiB" % host.get("ram_gib"))
    lines.append("- lsaca: %s (%s)" % (tools.get("lsaca"), tools.get("lsaca_compiler")))
    lines.append("- gcc: %s" % tools.get("gcc"))
    lines.append("- rustc: %s" % tools.get("rustc"))
    lines.append("- Python: %s" % tools.get("python"))
    lines.append("- Node.js: %s" % tools.get("node"))
    if tools.get("cmake"):
        lines.append("- cmake: %s" % tools["cmake"])
    if tools.get("ninja"):
        lines.append("- ninja: %s" % tools["ninja"])
    lines.append("- lsaca build mode: %s" % (results["lsaca_build"].get("mode") or "n/a"))
    lines.append("")
    lines.append(
        "## Compile time (seconds, median of %d runs; lower is better)"
        % results["settings"]["compile_repeats"]
    )
    lines.append("")
    lines.append(
        "| program | lsaca cold build | lsaca cached build | lsaca emit-c + gcc | "
        "gcc -O2 | rustc -O | Python 3 | Node.js |"
    )
    lines.append("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for program in programs:
        entry = results["programs"][program]["languages"]
        lsaca_compile = (entry.get("lsaca") or {}).get("compile") or {}
        if lsaca_compile.get("kind") == "lsaca-official":
            cold = fmt_seconds(lsaca_compile.get("cold_s"))
            cached = fmt_seconds(lsaca_compile.get("cached_median_s"))
            fallback = "n/a"
        elif lsaca_compile.get("kind") == "lsaca-fallback":
            cold = "n/a"
            cached = "n/a"
            fallback = fmt_seconds(lsaca_compile.get("total_s"))
        else:
            cold = cached = fallback = "n/a"
        c_compile = (entry.get("c") or {}).get("compile") or {}
        rust_compile = (entry.get("rust") or {}).get("compile") or {}
        row = [
            program,
            fmt_seconds(lsaca_compile.get("cold_s")),
            fmt_seconds(lsaca_compile.get("cached_median_s")),
            fallback,
            fmt_seconds(c_compile.get("median_s")),
            fmt_seconds(rust_compile.get("median_s")),
            "n/a",
            "n/a",
        ]
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")
    if results["lsaca_build"].get("mode") == "fallback":
        lines.append(
            "> lsaca fallback: the official CMake-based build could not be driven in this "
            "environment, so the lsaca numbers are lsaca emit-c plus a direct gcc -O2 -DNDEBUG "
            "compile of the generated C. The official cold/cached columns are shown as n/a."
        )
        lines.append("")
    lines.append("## Runtime (median (min), milliseconds; lower is better)")
    lines.append("")
    lines.append("| language | " + " | ".join(programs) + " |")
    lines.append("| --- | " + " | ".join("---:" for _ in programs) + " |")
    for language in languages:
        cells = [LANGUAGE_LABELS[language]]
        for program in programs:
            record = results["programs"][program]["languages"].get(language) or {}
            cells.append(fmt_runtime(record.get("runtime")))
        lines.append("| " + " | ".join(cells) + " |")
    lines.append("")
    lines.append("## Peak memory (MiB, peak working set; lower is better)")
    lines.append("")
    lines.append("| language | " + " | ".join(programs) + " |")
    lines.append("| --- | " + " | ".join("---:" for _ in programs) + " |")
    for language in languages:
        cells = [LANGUAGE_LABELS[language]]
        for program in programs:
            record = results["programs"][program]["languages"].get(language) or {}
            cells.append(fmt_memory(record.get("memory")))
        lines.append("| " + " | ".join(cells) + " |")
    lines.append("")
    lines.append("## Workloads")
    lines.append("")
    for program in programs:
        spec = results["programs"][program]
        lines.append(
            "- %s: %s; expected stdout %s"
            % (program, spec["description"], spec["expected"])
        )
    lines.append("")
    if results.get("warnings"):
        lines.append("## Warnings")
        lines.append("")
        for warning in results["warnings"]:
            lines.append("- " + warning)
        lines.append("")
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Cross-language benchmarks for the lsaca compiler.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--quick", action="store_true", help="fewer repetitions (3 runtime runs, 1 compile run)"
    )
    parser.add_argument("--runs", type=int, default=7, help="measured runtime runs per benchmark")
    parser.add_argument("--warmups", type=int, default=1, help="warm-up runtime runs per benchmark")
    parser.add_argument(
        "--compile-repeats",
        type=int,
        default=3,
        help="compiler invocations per native/fallback compile measurement",
    )
    parser.add_argument(
        "--build-mode",
        choices=["auto", "official", "fallback"],
        default="auto",
        help="lsaca build strategy: auto tries the official CMake build then falls back; "
        "official fails hard; fallback uses emit-c + gcc directly",
    )
    parser.add_argument(
        "--official-timeout",
        type=float,
        default=120.0,
        help="timeout in seconds for one official lsaca build",
    )
    parser.add_argument(
        "--run-timeout", type=float, default=180.0, help="timeout in seconds for one benchmark run"
    )
    parser.add_argument(
        "--compiler",
        default=None,
        help="path to the lsaca executable (default target/release/lsaca.exe)",
    )
    parser.add_argument(
        "--programs", default=None, help="comma separated subset of: " + ",".join(PROGRAMS)
    )
    parser.add_argument(
        "--languages", default=None, help="comma separated subset of: " + ",".join(LANGUAGES)
    )
    parser.add_argument(
        "--json",
        default=None,
        help="raw results JSON path (default benchmarks/results/latest.json)",
    )
    parser.add_argument(
        "--md", default=None, help="markdown tables path (default benchmarks/results/latest.md)"
    )
    parser.add_argument("--no-memory", action="store_true", help="skip peak memory measurement")
    parser.add_argument("--root", default=None, help="repository root (default: parent of this script)")
    parser.add_argument("--list", action="store_true", help="list programs and languages and exit")
    args = parser.parse_args(argv)
    if args.quick:
        args.runs = min(args.runs, 3)
        args.compile_repeats = 1
    if args.runs < 1:
        parser.error("--runs must be >= 1")
    if args.warmups < 0:
        parser.error("--warmups must be >= 0")
    if args.compile_repeats < 1:
        parser.error("--compile-repeats must be >= 1")
    if args.programs:
        args.programs = [item.strip() for item in args.programs.split(",") if item.strip()]
        unknown = [item for item in args.programs if item not in PROGRAMS]
        if unknown:
            parser.error("unknown program(s): %s" % ", ".join(unknown))
    if args.languages:
        args.languages = [item.strip() for item in args.languages.split(",") if item.strip()]
        unknown = [item for item in args.languages if item not in LANGUAGES]
        if unknown:
            parser.error("unknown language(s): %s" % ", ".join(unknown))
    return args


def main(argv=None):
    args = parse_args(argv)
    if args.list:
        print("programs: " + ", ".join(PROGRAMS))
        print("languages: " + ", ".join(LANGUAGES))
        return 0
    root = Path(args.root).resolve() if args.root else REPO_ROOT
    if not (root / "benchmarks" / "programs").is_dir():
        eprint("error: %s does not look like the lsaca repository root" % root)
        return 2
    args.root = str(root)
    harness = Harness(args)
    harness.compile_repeats = args.compile_repeats
    json_path = (
        Path(args.json).resolve() if args.json else root / "benchmarks" / "results" / "latest.json"
    )
    md_path = (
        Path(args.md).resolve() if args.md else root / "benchmarks" / "results" / "latest.md"
    )
    try:
        harness.ensure_compiler()
        eprint(
            "[bench] lsaca compiler: %s (%s)"
            % (rel_path(harness.compiler, root), harness.lsaca_version)
        )
        eprint(
            "[bench] tools: gcc=%s rustc=%s node=%s"
            % (harness.tools["gcc"], harness.tools["rustc"], harness.tools["node"])
        )
        eprint(
            "[bench] settings: runs=%d warmups=%d compile_repeats=%d build_mode=%s quick=%s"
            % (
                args.runs,
                args.warmups,
                harness.compile_repeats,
                args.build_mode,
                args.quick,
            )
        )
        results = harness.run()
    except HarnessError as exc:
        eprint("")
        eprint("BENCHMARK FAILED: %s" % exc)
        return 2
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(results, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    markdown = render_markdown(results)
    md_path.parent.mkdir(parents=True, exist_ok=True)
    md_path.write_text(markdown, encoding="utf-8")
    print(markdown, end="")
    eprint("")
    eprint("[bench] raw results: %s" % json_path)
    eprint("[bench] markdown:    %s" % md_path)
    if results["warnings"]:
        eprint("[bench] warnings:")
        for warning in results["warnings"]:
            eprint("[bench]   - " + warning)
    return 0


if __name__ == "__main__":
    sys.exit(main())
