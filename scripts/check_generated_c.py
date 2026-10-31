#!/usr/bin/env python3
"""check_generated_c.py -- prove that every generated C project is valid C17.

The script lowers every selected lsaca program to a C project, compiles that
project in strict warning mode under both build profiles, runs the executable,
and checks generated-code determinism and runtime cleanliness.

Selection (default):

  * every fixture in crates/lsaca/tests/programs/*.ls except names starting
    with bad_ (those are front-end rejection tests);
  * every examples/*.ls program.

The annotated negative fixtures (panic/argv/stderr fixtures and the two
documented compile-error fixtures iter_collect_no_type.ls and
iter_sum_non_numeric.ls) are intentionally selected: their expected failure keeps
the report honest about what the tree currently accepts. By default a run exits
0 when the only failures are annotated negatives; pass --strict to make any
annotated failure fail the run.

Usage:

  python scripts/check_generated_c.py
  python scripts/check_generated_c.py --list
  python scripts/check_generated_c.py --only refs --only hashmap
  python scripts/check_generated_c.py --profile release --jobs 4
  python scripts/check_generated_c.py --bin target/debug/lsaca.exe
  python scripts/check_generated_c.py --summary-only --jobs 4
  python scripts/check_generated_c.py --keep --json target/generated-c-report.json

Per program and per requested profile:

  1. emit the C project twice into two directories (release adds -r) and
     require both emit runs to exit 0;
  2. compare every generated file byte-for-byte between the two emissions
     (main.c, lsaca_rt.h, lsaca_rt_internal.h, the runtime .c files and
     CMakeLists.txt) to enforce deterministic code generation;
  3. compile every generated .c file with -std=c17 -Wall -Wextra -Wpedantic
     -Werror, adding -O2 -DNDEBUG for release and -O0 -g -DLSACA_DEBUG
     -DLSACA_DEBUG_RC for debug;
  4. run the executable with a 10 second timeout and require exit code 0;
  5. require stderr to be empty in both profiles (debug enables ORC leak
     tracking, so leak reports and panics both fail the run).

Failure categories: compiler-exit (front-end rejected the program),
project (emitted file set or CMake target does not match the contract),
emit (second emission failed), determinism, gcc-error, timeout, run-exit,
stderr.

Report: one PASS/FAIL line per program with elapsed milliseconds, a summary
line "checked N programs (M builds), passed X, failed Y", and a machine
readable JSON report (default target/generated-c-report.json) that lists each
program and profile with timings, digests, exit codes and the failing detail.

The work directory defaults to a fresh directory under target/generated-c/ and
is deleted unless --keep is given. TEMP, TMP, TMPDIR and LSACA_CACHE_DIR point
inside it for every child process.

Compiler discovery: LSACA_CC, then C:/msys64/ucrt64/bin/gcc.exe, then gcc on
PATH. When no usable compiler is found the script prints a clear message and
exits with code 77.

Compiler discovery for lsaca: --bin, else the first existing path among
target/release/lsaca.exe, target/release/lsaca, target/debug/lsaca.exe,
target/debug/lsaca, target/lead/release/lsaca. A --bin value may be a path or a
bare command name resolved through PATH.

Exit codes: 0 all checks passed, 1 at least one failure, 2 usage/setup error,
77 no C compiler available.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional, Sequence

# --- repository layout -----------------------------------------------------

SCRIPT_PATH = Path(__file__).resolve()
REPO_ROOT = SCRIPT_PATH.parent.parent
EXAMPLES_DIR = REPO_ROOT / "examples"
FIXTURES_DIR = REPO_ROOT / "crates" / "lsaca" / "tests" / "programs"
BAD_FIXTURE_PREFIX = "bad_"

# --- generated project contract (mirrors runtime::sources()) --------------

EXPECTED_RUNTIME_C_FILES = (
    "lsaca_rt.c",
    "lsaca_rt_string.c",
    "lsaca_rt_vec.c",
    "lsaca_rt_fmt.c",
    "lsaca_rt_map.c",
    "lsaca_rt_unicode.c",
)
EXPECTED_C_FILES = ("main.c",) + EXPECTED_RUNTIME_C_FILES
EXPECTED_PROJECT_FILES = frozenset(
    ("CMakeLists.txt", "lsaca_rt.h", "lsaca_rt_internal.h", "lsaca_rt_unicode.h")
    + EXPECTED_C_FILES
)
# Large programs are emitted as main.c plus up to 8 deterministic translation
# units named lsaca_unit_1.c .. lsaca_unit_N.c (see codegen task-42).
UNIT_PATTERN = re.compile(r"^lsaca_unit_([1-9][0-9]*)\.c$")


def split_unit_names(names: Sequence[str]) -> list:
    """Return the sorted lsaca_unit_N.c names present in a project file list."""
    numbered = []
    for name in names:
        match = UNIT_PATTERN.match(name)
        if match is not None:
            numbered.append((int(match.group(1)), name))
    numbered.sort()
    if [index for index, _ in numbered] != list(range(1, len(numbered) + 1)):
        raise StepFailed(
            "project",
            "split translation units are not contiguous from lsaca_unit_1.c: "
            + repr([name for _, name in numbered]),
        )
    return [name for _, name in numbered]


def project_source_names(names: Sequence[str]) -> list:
    """main.c first, then split units in order, then the runtime translation units."""
    return ["main.c"] + split_unit_names(names) + list(EXPECTED_RUNTIME_C_FILES)

# --- build profiles --------------------------------------------------------

WARNING_FLAGS = ("-std=c17", "-Wall", "-Wextra", "-Wpedantic", "-Werror")
PROFILE_FLAGS = {
    "release": WARNING_FLAGS + ("-O2", "-DNDEBUG"),
    "debug": WARNING_FLAGS + ("-O0", "-g", "-DLSACA_DEBUG", "-DLSACA_DEBUG_RC"),
}
PROFILE_ORDER = ("release", "debug")

# --- process limits and tool discovery ------------------------------------

RUN_TIMEOUT = 10.0
COMPILE_TIMEOUT = 180.0
EMIT_TIMEOUT = 120.0

EXE_SUFFIX = ".exe" if os.name == "nt" else ""
DEFAULT_COMPILER = "C:/msys64/ucrt64/bin/gcc.exe"
LSACA_CC_ENV = "LSACA_CC"
MSYS_BIN = "C:/msys64/ucrt64/bin"

LSACA_CANDIDATES = (
    "target/release/lsaca.exe",
    "target/release/lsaca",
    "target/debug/lsaca.exe",
    "target/debug/lsaca",
    "target/lead/release/lsaca.exe",
    "target/lead/release/lsaca",
    "target/lead/debug/lsaca.exe",
    "target/lead/debug/lsaca",
    "target/bench-tools/lsaca.exe",
    "target/bench-tools/lsaca",
)

DEFAULT_JSON = REPO_ROOT / "target" / "generated-c-report.json"
WORK_ROOT = REPO_ROOT / "target" / "generated-c"

LEAK_MARKERS = (b"reference-counted allocation", b"leaked")
DEFAULT_JOBS = os.cpu_count() or 1

# Fixtures that intentionally violate the generic success criteria: they read
# argv, write stderr on purpose, panic on purpose, or are documented
# compile-error fixtures. They are still checked strictly - they still make the
# run fail - but the report labels them so their harness failures are not
# mistaken for compiler/codegen regressions.
EXPECTED_FAILURE_REASONS = {
    "crates/lsaca/tests/programs/args.ls": "requires argv (e2e passes 'alpha beta')",
    "crates/lsaca/tests/programs/eprintln.ls": "writes 'warn: 1' to stderr by design",
    "crates/lsaca/tests/programs/overflow.ls": "panic fixture; e2e expects exit 101",
    "crates/lsaca/tests/programs/panic_bounds.ls": "panic fixture; e2e expects exit 101",
    "crates/lsaca/tests/programs/panic_div.ls": "panic fixture; e2e expects exit 101",
    "crates/lsaca/tests/programs/panic_unwrap.ls": "panic fixture; e2e expects exit 101",
    "crates/lsaca/tests/programs/panic_substring.ls": "panic fixture; e2e checks the char-boundary message",
    "crates/lsaca/tests/programs/vec_capacity_negative.ls": "panic fixture; e2e expects exit 101",
    "crates/lsaca/tests/programs/iter_collect_no_type.ls": "documented compile-error fixture (collect needs a known Vec<T>)",
    "crates/lsaca/tests/programs/iter_sum_non_numeric.ls": "documented compile-error fixture (sum supports only i64/f64)",
}


class UsageError(Exception):
    """A problem with the invocation or environment (exit code 2)."""


class NoCompilerError(Exception):
    """No usable C compiler was found (exit code 77)."""


class StepFailed(Exception):
    """One step of one profile failed."""

    def __init__(self, category: str, detail: str) -> None:
        super().__init__(detail)
        self.category = category
        self.detail = detail


# --- data model ------------------------------------------------------------

@dataclass(frozen=True)
class Source:
    path: Path
    display: str


@dataclass(frozen=True)
class Config:
    bin_path: Path
    bin_version: str
    compiler: Path
    compiler_version: str
    workdir: Path
    profile: str
    jobs: int
    keep: bool
    json_path: Path


@dataclass
class ProfileResult:
    profile: str
    ok: bool = False
    category: str = ""
    detail: str = ""
    emit_ms: int = 0
    emit_again_ms: int = 0
    determinism: bool = False
    digest: str = ""
    generated_files: list = field(default_factory=list)
    gcc_ms: int = 0
    run_ms: int = 0
    run_rc: Optional[int] = None
    stdout_bytes: int = 0
    stderr_bytes: int = 0
    stderr_head: str = ""


@dataclass
class ProgramResult:
    source: Source
    profiles: list = field(default_factory=list)
    expected: str = ""

    @property
    def ok(self) -> bool:
        return bool(self.profiles) and all(profile.ok for profile in self.profiles)

    @property
    def total_ms(self) -> int:
        return sum(
            profile.emit_ms
            + profile.emit_again_ms
            + profile.gcc_ms
            + profile.run_ms
            for profile in self.profiles
        )


# --- small helpers ---------------------------------------------------------

def configure_stdio() -> None:
    """Keep reporting robust when a path cannot be encoded by the console."""
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(errors="backslashreplace")  # type: ignore[attr-defined]
        except (AttributeError, OSError):
            pass


def one_line(data: bytes, limit: int = 400) -> str:
    """Compact single-line excerpt of subprocess output."""
    text = data.decode("utf-8", "replace")
    parts = [part.strip() for part in text.splitlines() if part.strip()]
    text = " | ".join(parts)
    if not text:
        return "(no output)"
    if len(text) > limit:
        return "..." + text[-limit:]
    return text


def display_path(path: Path) -> str:
    """Repository-relative POSIX path when possible."""
    try:
        return path.resolve().relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def source_name(source: Source) -> str:
    """Indexed directory name for one program."""
    safe = "".join(char if (char.isalnum() or char in "-_.") else "-" for char in source.path.stem)
    return safe or "program"


def find_compiler() -> Path:
    """Resolve the C compiler or raise NoCompilerError (exit code 77)."""
    override = os.environ.get(LSACA_CC_ENV, "").strip()
    if override:
        candidate = Path(override)
        if not candidate.is_absolute():
            candidate = Path.cwd() / candidate
        if not candidate.is_file():
            raise NoCompilerError(
                f"{LSACA_CC_ENV} points to {override}, which is not an existing file"
            )
        return candidate.resolve()
    default = Path(DEFAULT_COMPILER)
    if default.is_file():
        return default
    found = shutil.which("gcc")
    if found is not None:
        return Path(found).resolve()
    raise NoCompilerError(
        "no C compiler found: set " + LSACA_CC_ENV + " or install gcc "
        "(checked " + DEFAULT_COMPILER + " and PATH)"
    )


def probe_compiler(compiler: Path) -> str:
    """Run the compiler once so a broken toolchain fails before the full run."""
    try:
        completed = subprocess.run(
            [str(compiler), "--version"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30.0,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise NoCompilerError(f"C compiler {compiler} could not be executed: {exc}") from None
    if completed.returncode != 0:
        raise NoCompilerError(
            f"C compiler {compiler} --version failed with exit {completed.returncode}: "
            + one_line(completed.stderr)
        )
    lines = completed.stdout.decode("utf-8", "replace").strip().splitlines()
    return lines[0].strip()[:160] if lines else "(no version output)"


def resolve_bin(value: Optional[str]) -> Path:
    """Resolve --bin, accepting a path or a bare command name found on PATH."""
    if value:
        candidate = Path(value)
        if candidate.is_absolute():
            path_candidates = [candidate]
        else:
            path_candidates = [Path.cwd() / candidate]
        for entry in path_candidates:
            if entry.is_file():
                return entry.resolve()
        if not candidate.is_absolute() and candidate.parent == Path("."):
            on_path = shutil.which(value)
            if on_path is not None:
                return Path(on_path).resolve()
            raise UsageError(
                f"--bin is neither an existing file nor a command on PATH: {value}"
            )
        raise UsageError(f"--bin is not an existing file: {value}")
    for relative in LSACA_CANDIDATES:
        candidate = REPO_ROOT / relative
        if candidate.is_file():
            return candidate
    raise UsageError(
        "no lsaca executable found; look for target/release/lsaca.exe, "
        "target/debug/lsaca.exe or pass --bin PATH"
    )


def probe_bin(bin_path: Path) -> str:
    try:
        completed = subprocess.run(
            [str(bin_path), "version"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30.0,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise UsageError(f"lsaca executable {bin_path} could not be executed: {exc}") from None
    if completed.returncode != 0:
        raise UsageError(
            f"lsaca executable {bin_path} version failed with exit "
            f"{completed.returncode}: " + one_line(completed.stderr)
        )
    lines = completed.stdout.decode("utf-8", "replace").strip().splitlines()
    return lines[0].strip()[:160] if lines else "(no version output)"


def child_env(workdir: Path, compiler: Path) -> dict:
    """Environment for child processes: temp/cache inside the work directory."""
    env = dict(os.environ)
    tmp = workdir / "tmp"
    cache = workdir / "cache"
    env["TEMP"] = str(tmp)
    env["TMP"] = str(tmp)
    env["TMPDIR"] = str(tmp)
    env["LSACA_CACHE_DIR"] = str(cache)
    path_parts = [str(compiler.parent), MSYS_BIN, env.get("PATH", "")]
    env["PATH"] = os.pathsep.join(part for part in path_parts if part)
    return env


def run_command(
    argv: Sequence[object],
    cwd: Path,
    timeout: float,
    env: dict,
) -> tuple[subprocess.CompletedProcess, int]:
    """Run one subprocess; return it plus elapsed milliseconds."""
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            [str(item) for item in argv],
            cwd=str(cwd),
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        raise StepFailed("timeout", f"timed out after {timeout:g}s") from None
    except OSError as exc:
        raise StepFailed("spawn", f"could not execute {argv[0]}: {exc}") from None
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    return completed, elapsed_ms


# --- source selection ------------------------------------------------------

def select_sources(only: Sequence[str]) -> tuple[list, dict]:
    fixtures = sorted(FIXTURES_DIR.glob("*.ls"), key=lambda path: path.name)
    examples = sorted(EXAMPLES_DIR.glob("*.ls"), key=lambda path: path.name)
    sources = []
    excluded_bad = 0
    for path in fixtures:
        if path.name.startswith(BAD_FIXTURE_PREFIX):
            excluded_bad += 1
            continue
        sources.append(Source(path=path.resolve(), display=display_path(path)))
    for path in examples:
        sources.append(Source(path=path.resolve(), display=display_path(path)))
    if only:
        needles = [needle.lower() for needle in only]
        sources = [
            source
            for source in sources
            if any(needle in source.display.lower() for needle in needles)
        ]
        if not sources:
            raise UsageError("--only matched no selected programs: " + ", ".join(only))
    if not sources:
        raise UsageError("no programs selected")
    stats = {
        "examples": sum(1 for source in sources if source.display.startswith("examples/")),
        "fixtures": sum(1 for source in sources if not source.display.startswith("examples/")),
        "excluded_bad": excluded_bad,
        "selected": len(sources),
    }
    return sources, stats


# --- emitted project checks -------------------------------------------------

def parse_cmake_executable_sources(text: str) -> Optional[list]:
    """Return the source list of the first add_executable(...) command."""
    lines = text.splitlines()
    index = 0
    while index < len(lines):
        stripped = lines[index].strip()
        if stripped.startswith("add_executable("):
            body = stripped[len("add_executable("):]
            while ")" not in body and index + 1 < len(lines):
                index += 1
                body += " " + lines[index].strip()
            body = body.split(")", 1)[0]
            tokens = body.split()
            if len(tokens) < 2:
                return None
            return tokens[1:]
        index += 1
    return None


def validate_project(emit_dir: Path) -> list:
    """Assert the emitted file set and CMake target match the frozen contract."""
    try:
        names = sorted(path.name for path in emit_dir.iterdir() if path.is_file())
    except OSError as exc:
        raise StepFailed("project", f"cannot list emitted project {emit_dir}: {exc}") from None
    actual = set(names)
    units = split_unit_names(names)
    allowed = EXPECTED_PROJECT_FILES | set(units)
    missing = sorted(EXPECTED_PROJECT_FILES - actual)
    unexpected = sorted(actual - allowed)
    if missing or unexpected:
        raise StepFailed(
            "project",
            f"emitted file set mismatch in {emit_dir}: missing={missing} unexpected={unexpected}",
        )
    try:
        cmake_text = (emit_dir / "CMakeLists.txt").read_text(encoding="utf-8")
    except OSError as exc:
        raise StepFailed("project", f"cannot read emitted CMakeLists.txt: {exc}") from None
    cmake_sources = parse_cmake_executable_sources(cmake_text)
    if cmake_sources is None:
        raise StepFailed("project", "emitted CMakeLists.txt has no add_executable source list")
    expected_sources = project_source_names(names)
    if len(cmake_sources) != len(set(cmake_sources)) or set(cmake_sources) != set(
        expected_sources
    ):
        raise StepFailed(
            "project",
            f"CMake add_executable sources {cmake_sources} do not match the expected "
            f"translation units {expected_sources}",
        )
    return names


def emit_project(config: Config, source: Source, profile: str, emit_dir: Path, env: dict) -> int:
    """Emit one project; return elapsed milliseconds."""
    emit_dir.mkdir(parents=True, exist_ok=True)
    argv = [config.bin_path, "emit-c"]
    if profile == "release":
        argv.append("-r")
    argv.extend(["-o", emit_dir, source.path])
    completed, elapsed_ms = run_command(argv, REPO_ROOT, EMIT_TIMEOUT, env)
    if completed.returncode != 0:
        raise StepFailed(
            "compiler-exit",
            f"lsaca emit-c ({profile}) exited {completed.returncode}: "
            + one_line(completed.stderr),
        )
    if not (emit_dir / "main.c").is_file():
        raise StepFailed("emit", f"lsaca emit-c ({profile}) wrote no main.c in {emit_dir}")
    return elapsed_ms


def compare_emissions(dir_a: Path, dir_b: Path) -> tuple[str, list]:
    """Compare every generated file byte-for-byte; return digest and file list."""
    try:
        names_a = sorted(path.name for path in dir_a.iterdir() if path.is_file())
        names_b = sorted(path.name for path in dir_b.iterdir() if path.is_file())
    except OSError as exc:
        raise StepFailed("determinism", f"cannot list emitted projects: {exc}") from None
    if names_a != names_b:
        raise StepFailed(
            "determinism",
            f"generated file sets differ: A={names_a} B={names_b}",
        )
    digest = hashlib.sha256()
    for name in names_a:
        try:
            data_a = (dir_a / name).read_bytes()
            data_b = (dir_b / name).read_bytes()
        except OSError as exc:
            raise StepFailed("determinism", f"cannot read generated {name}: {exc}") from None
        if data_a != data_b:
            sha_a = hashlib.sha256(data_a).hexdigest()[:16]
            sha_b = hashlib.sha256(data_b).hexdigest()[:16]
            raise StepFailed(
                "determinism",
                f"generated file {name} differs: A {len(data_a)}B sha256={sha_a} "
                f"vs B {len(data_b)}B sha256={sha_b}",
            )
        digest.update(name.encode("utf-8"))
        digest.update(b"|")
        digest.update(hashlib.sha256(data_a).digest())
    return digest.hexdigest(), names_a


def compile_project(config: Config, emit_dir: Path, executable: Path, profile: str, env: dict) -> int:
    """Compile the generated project; return elapsed milliseconds."""
    flags = PROFILE_FLAGS[profile]
    try:
        present = sorted(path.name for path in emit_dir.iterdir() if path.is_file())
    except OSError as exc:
        raise StepFailed("gcc-error", f"cannot list emitted project {emit_dir}: {exc}") from None
    sources = [emit_dir / name for name in project_source_names(present)]
    argv = [
        config.compiler,
        *flags,
        "-I",
        emit_dir,
        *sources,
        "-lm",
        "-o",
        executable,
    ]
    completed, elapsed_ms = run_command(argv, emit_dir, COMPILE_TIMEOUT, env)
    if completed.returncode != 0:
        raise StepFailed(
            "gcc-error",
            f"gcc ({profile}) exited {completed.returncode}: "
            + one_line(completed.stderr),
        )
    if b"warning:" in completed.stderr:
        raise StepFailed(
            "gcc-error",
            f"gcc ({profile}) reported warnings despite -Werror: "
            + one_line(completed.stderr),
        )
    if not executable.is_file():
        raise StepFailed("gcc-error", f"gcc ({profile}) produced no executable at {executable}")
    return elapsed_ms


# --- one program, one or two profiles -------------------------------------

def check_profile(
    config: Config,
    source: Source,
    profile: str,
    program_dir: Path,
    env: dict,
) -> ProfileResult:
    """Verify one program under one profile; never raises for a check failure."""
    result = ProfileResult(profile=profile)
    profile_dir = program_dir / profile
    emit_a = profile_dir / "emit-a"
    emit_b = profile_dir / "emit-b"
    try:
        result.emit_ms = emit_project(config, source, profile, emit_a, env)
        validate_project(emit_a)
        result.emit_again_ms = emit_project(config, source, profile, emit_b, env)
        result.digest, result.generated_files = compare_emissions(emit_a, emit_b)
        result.determinism = True

        executable = profile_dir / ("program-" + profile + EXE_SUFFIX)
        result.gcc_ms = compile_project(config, emit_a, executable, profile, env)

        completed, result.run_ms = run_command([executable], program_dir, RUN_TIMEOUT, env)
        result.run_rc = completed.returncode
        result.stdout_bytes = len(completed.stdout)
        result.stderr_bytes = len(completed.stderr)
        result.stderr_head = one_line(completed.stderr)
        if completed.returncode != 0:
            raise StepFailed(
                "run-exit",
                f"exit {completed.returncode}: stderr {one_line(completed.stderr)}",
            )
        if completed.stderr:
            lowered = completed.stderr.lower()
            leak = any(marker in lowered for marker in LEAK_MARKERS)
            label = "LEAK" if leak else "stderr"
            raise StepFailed(
                "stderr",
                f"{label}: {len(completed.stderr)} byte(s) on stderr: "
                + one_line(completed.stderr),
            )
        result.ok = True
    except StepFailed as exc:
        result.category = exc.category
        result.detail = exc.detail
    return result


def check_program(config: Config, source: Source, index: int) -> ProgramResult:
    """Verify one program under every requested profile."""
    program_dir = config.workdir / "programs" / (f"{index:03d}-" + source_name(source))
    env = child_env(config.workdir, config.compiler)
    profiles = list(PROFILE_ORDER) if config.profile == "both" else [config.profile]
    result = ProgramResult(source=source)
    result.expected = EXPECTED_FAILURE_REASONS.get(source.display, "")
    rejected_in: Optional[str] = None
    for profile in profiles:
        if rejected_in is not None:
            skipped = ProfileResult(profile=profile)
            skipped.category = "compiler-exit"
            skipped.detail = f"not attempted: compiler rejected the source in the {rejected_in} profile"
            result.profiles.append(skipped)
            continue
        profile_result = check_profile(config, source, profile, program_dir, env)
        result.profiles.append(profile_result)
        if profile_result.category == "compiler-exit":
            rejected_in = profile
    return result


def run_checks(config: Config, sources: Sequence[Source]) -> list:
    """Check every program in parallel; return results in source order.

    A thread pool is used rather than a process pool: workers only spawn
    subprocesses (emit-c, gcc, the generated executable) and wait on them, so
    the GIL is irrelevant, while multiprocessing's IPC pipes are unavailable
    in restricted Windows sandboxes (WinError 5 opening the pipe).
    """
    results: list = [None] * len(sources)
    with concurrent.futures.ThreadPoolExecutor(max_workers=config.jobs) as pool:
        futures = {
            pool.submit(check_program, config, source, index): index
            for index, source in enumerate(sources)
        }
        for future in concurrent.futures.as_completed(futures):
            index = futures[future]
            try:
                results[index] = future.result()
            except Exception as exc:  # defensive: never lose the report on one worker
                failed = ProgramResult(source=sources[index])
                profile = ProfileResult(profile="both")
                profile.category = "internal"
                profile.detail = type(exc).__name__ + ": " + str(exc)
                failed.profiles.append(profile)
                results[index] = failed
    return [result for result in results if result is not None]


# --- reporting -------------------------------------------------------------

def profile_status(profile: ProfileResult) -> str:
    return "ok" if profile.ok else (profile.category or "fail")


def format_program_line(result: ProgramResult) -> str:
    if result.ok:
        statuses = " ".join(f"{profile.profile}=ok" for profile in result.profiles)
        return f"PASS {result.source.display} {statuses} total={result.total_ms}ms"
    statuses = " ".join(
        f"{profile.profile}={profile_status(profile)}" for profile in result.profiles
    )
    first_failure = next(profile for profile in result.profiles if not profile.ok)
    suffix = f" [expected: {result.expected}]" if result.expected else ""
    return (
        f"FAIL {result.source.display} {statuses} "
        f"detail=<{first_failure.profile}: {first_failure.detail}>{suffix}"
    )


def build_report(
    config: Config,
    sources: Sequence[Source],
    stats: dict,
    results: Sequence[ProgramResult],
    started_at: datetime,
    elapsed_ms: int,
) -> dict:
    programs = []
    categories: dict = {}
    passed_profiles = 0
    failed_profiles = 0
    for result in results:
        entries = []
        for profile in result.profiles:
            entries.append(
                {
                    "profile": profile.profile,
                    "ok": profile.ok,
                    "category": profile.category,
                    "detail": profile.detail,
                    "emit_ms": profile.emit_ms,
                    "emit_again_ms": profile.emit_again_ms,
                    "determinism": profile.determinism,
                    "digest": ("sha256:" + profile.digest) if profile.digest else "",
                    "generated_files": profile.generated_files,
                    "gcc_ms": profile.gcc_ms,
                    "run_ms": profile.run_ms,
                    "run_rc": profile.run_rc,
                    "stdout_bytes": profile.stdout_bytes,
                    "stderr_bytes": profile.stderr_bytes,
                    "stderr_head": profile.stderr_head,
                }
            )
            if profile.ok:
                passed_profiles += 1
            else:
                failed_profiles += 1
                categories[profile.category] = categories.get(profile.category, 0) + 1
        programs.append(
            {
                "path": result.source.display,
                "ok": result.ok,
                "expected": result.expected,
                "total_ms": result.total_ms,
                "profiles": entries,
            }
        )
    failed_programs = sum(1 for result in results if not result.ok)
    expected_failures = [result for result in results if not result.ok and result.expected]
    unexpected_failures = [result for result in results if not result.ok and not result.expected]
    return {
        "schema": 1,
        "generated_at": started_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "elapsed_ms": elapsed_ms,
        "repo_root": str(REPO_ROOT),
        "bin": {"path": str(config.bin_path), "version": config.bin_version},
        "compiler": {"path": str(config.compiler), "version": config.compiler_version},
        "profile": config.profile,
        "jobs": config.jobs,
        "workdir": str(config.workdir),
        "sources": stats,
        "summary": {
            "programs": len(results),
            "builds": len(results) * len(PROFILE_ORDER if config.profile == "both" else [config.profile]),
            "passed_programs": len(results) - failed_programs,
            "failed_programs": failed_programs,
            "passed_builds": passed_profiles,
            "failed_builds": failed_profiles,
            "categories": dict(sorted(categories.items())),
            "expected_failures": len(expected_failures),
            "unexpected_failures": len(unexpected_failures),
            "expected_failure_programs": [result.source.display for result in expected_failures],
            "unexpected_failure_programs": [
                result.source.display for result in unexpected_failures
            ],
        },
        "programs": programs,
    }


def write_report(json_path: Path, report: dict) -> None:
    json_path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(report, indent=2, ensure_ascii=False)
    json_path.write_text(text + chr(10), encoding="utf-8")


# --- work directory --------------------------------------------------------

def create_workdir() -> Path:
    """Create a fresh work directory.

    tempfile.mkdtemp is deliberately not used: in this sandbox (and under some
    Windows ACL configurations) the directory it creates denies the current
    process further writes, so explicit mkdir calls are used instead.
    """
    try:
        WORK_ROOT.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise UsageError(f"cannot create work directory root {WORK_ROOT}: {exc}") from None
    stamp = time.strftime("%Y%m%d-%H%M%S")
    for attempt in range(1000):
        name = f"run-{stamp}-{os.getpid()}"
        if attempt:
            name += "-" + str(attempt)
        candidate = WORK_ROOT / name
        try:
            candidate.mkdir()
        except FileExistsError:
            continue
        except OSError as exc:
            raise UsageError(f"cannot create work directory under {WORK_ROOT}: {exc}") from None
        try:
            (candidate / "tmp").mkdir()
            (candidate / "cache").mkdir()
        except OSError as exc:
            raise UsageError(f"cannot initialise work directory {candidate}: {exc}") from None
        return candidate
    raise UsageError(f"cannot create a unique work directory under {WORK_ROOT}")


# --- command line ----------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="check_generated_c.py",
        description=(
            "Emit every selected lsaca program to C, verify deterministic "
            "codegen, compile it with strict warnings under the release and "
            "debug profiles, run it, and require clean stderr."
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "examples:" + chr(10)
            + "  python scripts/check_generated_c.py" + chr(10)
            + "  python scripts/check_generated_c.py --list" + chr(10)
            + "  python scripts/check_generated_c.py --only refs --only hashmap" + chr(10)
            + "  python scripts/check_generated_c.py --profile release --jobs 4" + chr(10)
            + "  python scripts/check_generated_c.py --keep --json target/generated-c-report.json"
            + chr(10) + chr(10)
            + "exit codes: 0 all checks passed (annotated known-negative fixtures may\n"
            + "  still fail and are reported), 1 at least one unexpected failure "
            + "(--strict: any failure), 2 usage/setup error, 77 no C compiler available"
        ),
    )
    parser.add_argument(
        "--list",
        dest="list_only",
        action="store_true",
        help="list the selected programs and exit without running any tool",
    )
    parser.add_argument(
        "--only",
        metavar="SUBSTR",
        action="append",
        default=None,
        help="keep only programs whose path contains SUBSTR (repeatable)",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        default=DEFAULT_JOBS,
        metavar="N",
        help=f"programs checked in parallel (default: {DEFAULT_JOBS})",
    )
    parser.add_argument(
        "--bin",
        metavar="PATH",
        help=(
            "lsaca executable, as a path or a bare command name resolved via PATH "
            "(default: first existing build artifact under target/)"
        ),
    )
    parser.add_argument(
        "--keep",
        action="store_true",
        help="keep the temporary work directory (removed by default)",
    )
    parser.add_argument(
        "--summary-only",
        dest="summary_only",
        action="store_true",
        help="print only the header and summary, not one line per program",
    )
    parser.add_argument(
        "--profile",
        choices=("release", "debug", "both"),
        default="both",
        help="build profile(s) to verify (default: both)",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help=(
            "fail the run if an annotated known-negative fixture fails; by default "
            "only unexpected failures make the run fail"
        ),
    )
    parser.add_argument(
        "--json",
        metavar="PATH",
        default=None,
        help=f"machine-readable report path (default: {display_path(DEFAULT_JSON)})",
    )
    return parser


def resolve_json_path(value: Optional[str]) -> Path:
    if value is None:
        return DEFAULT_JSON
    candidate = Path(value)
    if not candidate.is_absolute():
        candidate = Path.cwd() / candidate
    return candidate.resolve()


def main(argv: Optional[Sequence[str]] = None) -> int:
    configure_stdio()
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.jobs < 1:
        print("check_generated_c: error: --jobs must be at least 1", file=sys.stderr)
        return 2
    try:
        sources, stats = select_sources(args.only or [])
    except UsageError as exc:
        print(f"check_generated_c: error: {exc}", file=sys.stderr)
        return 2

    if args.list_only:
        for source in sources:
            print(f"LIST {source.display}")
        print(
            f"LIST total={stats['selected']} (examples={stats['examples']} "
            f"fixtures={stats['fixtures']} excluded_bad={stats['excluded_bad']})"
        )
        return 0

    try:
        compiler = find_compiler()
        compiler_version = probe_compiler(compiler)
    except NoCompilerError as exc:
        print(f"check_generated_c: {exc}", file=sys.stderr)
        print("check_generated_c: skipping generated-C checks (exit 77)")
        return 77

    try:
        bin_path = resolve_bin(args.bin)
        bin_version = probe_bin(bin_path)
        workdir = create_workdir()
    except UsageError as exc:
        print(f"check_generated_c: error: {exc}", file=sys.stderr)
        return 2

    json_path = resolve_json_path(args.json)
    config = Config(
        bin_path=bin_path,
        bin_version=bin_version,
        compiler=compiler,
        compiler_version=compiler_version,
        workdir=workdir,
        profile=args.profile,
        jobs=args.jobs,
        keep=args.keep,
        json_path=json_path,
    )

    profiles_text = "release+debug" if config.profile == "both" else config.profile
    print("check_generated_c: generated C project verification")
    print(f"lsaca: {config.bin_path} [{config.bin_version}]")
    print(f"compiler: {config.compiler} [{config.compiler_version}]")
    print(
        f"programs: {stats['selected']} (examples={stats['examples']} "
        f"fixtures={stats['fixtures']} excluded_bad={stats['excluded_bad']})"
    )
    print(f"profiles: {profiles_text}; jobs: {config.jobs}")
    print(f"workdir: {config.workdir}")
    print()

    started_at = datetime.now(timezone.utc)
    started = time.perf_counter()
    try:
        results = run_checks(config, sources)
    finally:
        pass
    elapsed_ms = int((time.perf_counter() - started) * 1000)

    if not args.summary_only:
        for result in results:
            print(format_program_line(result))
        print()

    report = build_report(config, sources, stats, results, started_at, elapsed_ms)
    summary = report["summary"]
    summary_line = (
        f"checked {summary['programs']} programs ({summary['builds']} builds), "
        f"passed {summary['passed_programs']}, failed {summary['failed_programs']}"
    )
    print(summary_line)
    if summary["categories"]:
        categories = ", ".join(
            f"{name}={count}" for name, count in summary["categories"].items()
        )
        print("failure categories: " + categories)
    if summary["expected_failure_programs"]:
        print(
            f"expected failures: {summary['expected_failures']} known-negative fixture(s) "
            + (
                "(--strict: counted as failures)"
                if getattr(args, "strict", False)
                else "(reported but not failing; pass --strict to enforce)"
            )
        )
        print(f"unexpected failures: {summary['unexpected_failures']} program(s) need triage")
    print(f"elapsed: {elapsed_ms}ms")

    json_ok = True
    try:
        write_report(json_path, report)
        print(f"report: {json_path}")
    except OSError as exc:
        json_ok = False
        print(f"check_generated_c: error: cannot write JSON report {json_path}: {exc}", file=sys.stderr)

    if config.keep:
        print(f"workdir kept: {config.workdir}")
    else:
        shutil.rmtree(config.workdir, ignore_errors=True)

    strict = getattr(args, "strict", False)
    failing = summary["failed_programs"] if strict else summary["unexpected_failures"]
    ok = not failing and json_ok
    print("GENERATED-C OK" if ok else "GENERATED-C FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
