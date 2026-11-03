#!/usr/bin/env python3
"""Functional and scale checks for the lsaca benchmark suite.

The programs under benchmarks/programs/checks/ are small self-contained lsaca
programs that verify algorithm behaviour (closed forms, sort invariants,
implementation equivalence, collection consistency, string round trips). They
assert their own invariants with assert!/assert_eq! and print one deterministic
line; this runner rebuilds each of them with the lsaca compiler, runs the
listed argument sets and compares stdout byte for byte, so a broken codegen or
type checker fails here as well.

The scale checksum lines are recomputed with the plain Python reference below
(never captured from a run), which keeps the check independent of the compiler
under test.

Usage (from the repository root):

    python scripts/bench_checks.py --compiler target/release/lsaca.exe
    python scripts/bench_checks.py --compiler ... --debug --warnings-as-errors
    python scripts/bench_checks.py --list

The runner uses the same build strategy as scripts/bench.py: the default
lsaca build first, and on failure 'lsaca emit-c' plus a direct gcc compile.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
CHECKS_DIR = REPO_ROOT / "benchmarks" / "programs" / "checks"
BUILD_ROOT = REPO_ROOT / "target" / "bench-checks"
MODULUS = 1000000007


def weighted_checksum(n):
    """Fold 1..=n the same way benchmarks/programs/checks/checks_loop_sum_scale.ls does."""
    checksum = 0
    for index in range(1, n + 1):
        checksum = (checksum * 31 + index) % MODULUS
    return checksum


def scale_expected(n):
    return "checksum={0}".format(weighted_checksum(n))


CHECKS = [
    {
        "file": "checks_loop_sum.ls",
        "description": "closed form for sum_to(n) across a grid of sizes",
        "cases": [
            {"scale": "default", "args": [], "expected": "ok loop_sum"},
        ],
    },
    {
        "file": "checks_loop_sum_scale.ls",
        "description": "argument-driven loop_sum small/medium/large scales",
        "cases": [
            {"scale": "small", "args": ["10"], "expected": scale_expected(10)},
            {"scale": "medium", "args": ["100000"], "expected": scale_expected(100000)},
            {"scale": "large", "args": ["1000000"], "expected": scale_expected(1000000)},
        ],
    },
    {
        "file": "checks_sort_invariants.ls",
        "description": "insertion sort preserves length/sum/min/max and stays ordered",
        "cases": [
            {"scale": "default", "args": [], "expected": "ok sort"},
        ],
    },
    {
        "file": "checks_fib_relations.ls",
        "description": "recursive, iterative and memoized fibonacci agree",
        "cases": [
            {"scale": "default", "args": [], "expected": "ok fib"},
        ],
    },
    {
        "file": "checks_hashmap_counts.ls",
        "description": "HashMap frequencies agree with a flat array and count totals",
        "cases": [
            {"scale": "default", "args": [], "expected": "ok hashmap"},
        ],
    },
    {
        "file": "checks_string_roundtrip.ls",
        "description": "string build/split/slice/upper/parse keep lengths and values",
        "cases": [
            {"scale": "default", "args": [], "expected": "ok strings"},
        ],
    },
]


class CheckError(RuntimeError):
    pass


def eprint(message):
    print(message, file=sys.stderr, flush=True)


def describe_failure(proc):
    parts = []
    if proc.stdout and proc.stdout.strip():
        parts.append("stdout:\n" + proc.stdout.strip())
    if proc.stderr and proc.stderr.strip():
        parts.append("stderr:\n" + proc.stderr.strip())
    text = "\n".join(parts) or "(no output)"
    lines = text.splitlines()
    return "\n".join(lines[-12:])


def known_tool_dirs():
    if os.name != "nt":
        return []
    candidates = [
        Path(r"C:\msys64\ucrt64\bin"),
        Path(r"C:\msys64\mingw64\bin"),
        Path.home() / ".cargo" / "bin",
    ]
    return [str(path) for path in candidates if path.is_dir()]


def build_child_env():
    env = dict(os.environ)
    tmp = BUILD_ROOT / "tmp"
    tmp.mkdir(parents=True, exist_ok=True)
    env["TEMP"] = env["TMP"] = env["TMPDIR"] = str(tmp)
    path_parts = known_tool_dirs()
    if env.get("PATH"):
        path_parts.append(env["PATH"])
    env["PATH"] = os.pathsep.join(path_parts)
    for key in ("LSACA_CACHE_DIR", "LSACA_CC", "LSACA_CFLAGS"):
        env.pop(key, None)
    return env


def run_command(cmd, env, cwd=None, timeout=300.0):
    return subprocess.run(
        [str(part) for part in cmd],
        cwd=str(cwd) if cwd is not None else None,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
    )


def find_gcc(env):
    found = shutil.which("gcc", path=env.get("PATH"))
    if found:
        return found
    return shutil.which("cc", path=env.get("PATH"))


def find_exe(outdir, stem):
    names = [stem + ".exe"] if os.name == "nt" else [stem, stem + ".exe"]
    for name in names:
        candidate = outdir / name
        if candidate.is_file():
            return candidate
    for path in sorted(outdir.iterdir()):
        if not path.is_file():
            continue
        if path.suffix.lower() == ".exe":
            return path
        if os.name != "nt" and not path.suffix and os.access(path, os.X_OK):
            return path
    return None


class CheckRunner:
    def __init__(self, args):
        self.args = args
        self.env = build_child_env()
        self.gcc = find_gcc(self.env)
        self.compiler = Path(args.compiler).resolve()
        if not self.compiler.is_file():
            raise CheckError("lsaca compiler not found: {0}".format(self.compiler))
        BUILD_ROOT.mkdir(parents=True, exist_ok=True)
        shutil.rmtree(BUILD_ROOT / "build", ignore_errors=True)
        shutil.rmtree(BUILD_ROOT / "cache", ignore_errors=True)
        (BUILD_ROOT / "build").mkdir(parents=True, exist_ok=True)
        self.results = []
        self.failures = []
        self.version = None

    def compiler_version(self):
        if self.version is None:
            proc = run_command([self.compiler, "version"], self.env, timeout=30)
            text = (proc.stdout or proc.stderr or "").strip().splitlines()
            self.version = text[0].strip() if text else "unknown"
        return self.version

    def fresh_cache_dir(self, label):
        path = BUILD_ROOT / "cache" / "{0}-{1}-{2}".format(label, os.getpid(), time.time_ns())
        path.mkdir(parents=True, exist_ok=False)
        return path

    def build_check(self, check, outdir, debug):
        """Build one check program and return its executable path."""
        stem = Path(check["file"]).stem
        src = CHECKS_DIR / check["file"]
        if not src.is_file():
            raise CheckError("missing check program: {0}".format(src))
        mode = self.args.build_mode
        if debug and mode == "official":
            raise CheckError("--build-mode official cannot build the debug (-DLSACA_DEBUG) variant; use auto or fallback")
        if mode in ("auto", "official") and not debug:
            try:
                return self.build_official(stem, src, outdir)
            except CheckError as exc:
                if mode == "official":
                    raise
                eprint("[checks] official build failed for {0}; using emit-c + gcc: {1}".format(stem, str(exc).splitlines()[0]))
        return self.build_fallback(stem, src, outdir, debug)

    def build_official(self, stem, src, outdir):
        outdir.mkdir(parents=True, exist_ok=True)
        env = dict(self.env)
        env["LSACA_CACHE_DIR"] = str(self.fresh_cache_dir(stem + "-official"))
        cmd = [self.compiler, "build", "-r", str(src), "-o", str(outdir)]
        proc = run_command(cmd, env, cwd=REPO_ROOT, timeout=120)
        if proc.returncode != 0:
            raise CheckError("lsaca build failed for {0}:\n{1}".format(stem, describe_failure(proc)))
        exe = find_exe(outdir, stem)
        if exe is None:
            raise CheckError("lsaca build succeeded for {0} but produced no executable".format(stem))
        return exe

    def build_fallback(self, stem, src, outdir, debug):
        if not self.gcc:
            raise CheckError("gcc (or cc) is required for the emit-c fallback build path")
        gen = outdir / "generated"
        shutil.rmtree(gen, ignore_errors=True)
        outdir.mkdir(parents=True, exist_ok=True)
        env = dict(self.env)
        env["LSACA_CACHE_DIR"] = str(self.fresh_cache_dir(stem + "-emit"))
        emit = [self.compiler, "emit-c"]
        if not debug:
            emit.append("-r")
        emit.extend([str(src), "-o", str(gen)])
        proc = run_command(emit, env, cwd=REPO_ROOT, timeout=120)
        if proc.returncode != 0:
            raise CheckError("lsaca emit-c failed for {0}:\n{1}".format(stem, describe_failure(proc)))
        for required in ("main.c", "lsaca_rt.h", "lsaca_rt.c"):
            if not (gen / required).is_file():
                raise CheckError("lsaca emit-c did not produce {0} for {1}".format(required, stem))
        generated_c = sorted(gen.glob("*.c"), key=lambda path: path.name)
        if not generated_c:
            raise CheckError("lsaca emit-c produced no .c files for {0}".format(stem))
        exe = outdir / (stem + (".exe" if os.name == "nt" else ""))
        cmd = [self.gcc, "-std=c17", "-Wall", "-Wextra"]
        if self.args.warnings_as_errors:
            cmd.append("-Werror")
        cmd.extend(["-O0", "-g", "-DLSACA_DEBUG"] if debug else ["-O2", "-DNDEBUG"])
        cmd.extend(str(path) for path in generated_c)
        cmd.extend(["-lm", "-o", str(exe)])
        proc = run_command(cmd, env, cwd=gen, timeout=300)
        if proc.returncode != 0:
            raise CheckError("gcc failed to compile the C generated by lsaca for {0}:\n{1}".format(stem, describe_failure(proc)))
        return exe

    def run_case(self, check, case, exe, debug):
        cmd = [str(exe)] + list(case["args"])
        proc = run_command(cmd, self.env, cwd=REPO_ROOT, timeout=120)
        label = "{0} [{1}]{2}".format(check["file"], case["scale"], " (debug)" if debug else "")
        failure = None
        if proc.returncode != 0:
            failure = "{0}: exit {1}".format(label, proc.returncode)
        elif proc.stdout.strip() != case["expected"].strip():
            failure = "{0}: stdout mismatch: expected {1!r}, got {2!r}".format(label, case["expected"], proc.stdout.strip())
        elif proc.stderr.strip():
            failure = "{0}: unexpected stderr: {1!r}".format(label, proc.stderr.strip()[-400:])
        record = {
            "file": check["file"],
            "scale": case["scale"],
            "args": list(case["args"]),
            "debug": debug,
            "expected": case["expected"],
            "ok": failure is None,
        }
        self.results.append(record)
        if failure is None:
            eprint("PASS " + label)
        else:
            eprint("FAIL " + failure)
            self.failures.append(failure)

    def run(self):
        selected = CHECKS
        if self.args.files:
            wanted = set(self.args.files)
            selected = [check for check in CHECKS if check["file"] in wanted or Path(check["file"]).stem in wanted]
            missing = wanted - {check["file"] for check in selected} - {Path(check["file"]).stem for check in selected}
            if missing:
                raise CheckError("unknown check(s): {0}".format(", ".join(sorted(missing))))
        eprint("[checks] compiler: {0} ({1})".format(self.compiler, self.compiler_version()))
        eprint("[checks] gcc: {0}".format(self.gcc or "missing"))
        start = time.time()
        for check in selected:
            modes = [False, True] if self.args.debug else [False]
            for debug in modes:
                outdir = BUILD_ROOT / "build" / Path(check["file"]).stem / ("debug" if debug else "release")
                exe = self.build_check(check, outdir, debug)
                eprint("[checks] built {0}{1} -> {2}".format(check["file"], " (debug)" if debug else "", exe.name))
                cases = check["cases"]
                if self.args.quick and len(cases) > 1:
                    cases = cases[:1]
                for case in cases:
                    self.run_case(check, case, exe, debug)
        elapsed = time.time() - start
        passed = sum(1 for record in self.results if record["ok"])
        total = len(self.results)
        eprint("[checks] {0}/{1} cases passed in {2:.1f} s".format(passed, total, elapsed))
        if self.args.json:
            payload = {
                "schema_version": 1,
                "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
                "compiler": str(self.compiler),
                "compiler_version": self.compiler_version(),
                "checks": self.results,
                "passed": passed,
                "total": total,
            }
            out = Path(self.args.json)
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
            eprint("[checks] wrote {0}".format(out))
        if self.failures:
            eprint("[checks] FAILED: {0} case(s); first failure: {1}".format(len(self.failures), self.failures[0]))
            return 1
        return 0


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Compile and run the lsaca benchmark check programs under benchmarks/programs/checks/.",
    )
    parser.add_argument("--compiler", default=None, help="path to the lsaca compiler (default: target/release/lsaca[.exe])")
    parser.add_argument("--build-mode", choices=("auto", "official", "fallback"), default="auto", help="how to build each check (default: auto)")
    parser.add_argument("--debug", action="store_true", help="also build with -O0 -g -DLSACA_DEBUG and require empty stderr (leak warnings fail)")
    parser.add_argument("--warnings-as-errors", action="store_true", help="compile the generated C with -Werror")
    parser.add_argument("--quick", action="store_true", help="run only the first case of each check")
    parser.add_argument("--json", default=None, help="write a JSON result report to this path")
    parser.add_argument("--files", default=None, help="comma-separated check file names or stems to run")
    parser.add_argument("--list", action="store_true", help="list the checks and their cases, then exit")
    args = parser.parse_args(argv)
    if args.compiler is None:
        default_name = "lsaca.exe" if os.name == "nt" else "lsaca"
        args.compiler = str(REPO_ROOT / "target" / "release" / default_name)
    if args.files:
        args.files = [part.strip() for part in args.files.split(",") if part.strip()]
    return args


def main(argv=None):
    args = parse_args(argv)
    if args.list:
        print("checks:")
        for check in CHECKS:
            print("  {0}: {1}".format(check["file"], check["description"]))
            for case in check["cases"]:
                print("    {0}: args={1} expected={2}".format(case["scale"], " ".join(case["args"]) or "(default)", case["expected"]))
        return 0
    try:
        runner = CheckRunner(args)
        return runner.run()
    except CheckError as exc:
        eprint("[checks] FAILED: {0}".format(exc))
        return 2


if __name__ == "__main__":
    sys.exit(main())
