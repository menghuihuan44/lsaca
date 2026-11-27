#!/usr/bin/env python3
"""Run the example programs through lsaca and their C counterparts.

Every case is compared byte for byte with the golden files used by the
real_programs_match_expected_output end-to-end test:

    crates/lsaca/tests/real_programs/expected/{json,json_file,markdown,
    markdown_file,lisp,lisp_file}.out

The lsaca cases invoke "lsaca run <program>.ls [-- <args>]" from
examples/programs, require exit status 0 and an empty stderr, normalize CRLF to
LF, and compare the stdout bytes exactly.  Unless --no-c is given, each C
reference program is also compiled once with

    gcc -std=c17 -O2 -Wall -Wextra -Werror -lm

into target/real-programs/bin and its stdout must match the same golden file.

Usage:
    python scripts/real_programs.py --bin target/release/lsaca
    python scripts/real_programs.py --bin target/release/lsaca --no-c --json out.json

Exit status: 0 when every case passes, 1 otherwise.
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

PROGRAMS = ("json", "markdown", "lisp")

CASES = (
    ("json", "json", [], "json.out"),
    ("json_file", "json", ["data/sample.json"], "json_file.out"),
    ("markdown", "markdown", [], "markdown.out"),
    ("markdown_file", "markdown", ["data/sample.md"], "markdown_file.out"),
    ("lisp", "lisp", [], "lisp.out"),
    ("lisp_file", "lisp", ["data/sample.lisp"], "lisp_file.out"),
)


def normalize(data: bytes) -> bytes:
    return data.replace(b"\r\n", b"\n")


def describe_mismatch(actual: bytes, expected: bytes) -> str:
    if actual == expected:
        return ""
    limit = min(len(actual), len(expected))
    offset = 0
    while offset < limit and actual[offset] == expected[offset]:
        offset += 1
    if offset >= limit:
        return (
            "stdout length mismatch: expected %d bytes, got %d bytes"
            % (len(expected), len(actual))
        )
    window = 20
    start = max(0, offset - window)
    a = actual[start : offset + window]
    e = expected[start : offset + window]
    return (
        "stdout mismatch at byte %d: expected %d bytes, got %d bytes; "
        "expected %r, got %r"
        % (offset, len(expected), len(actual), e, a)
    )


def find_gcc():
    candidate = shutil.which("gcc")
    if candidate:
        return candidate
    fallback = Path(r"C:\msys64\ucrt64\bin\gcc.exe")
    if fallback.is_file():
        return str(fallback)
    return None


def run_command(command, cwd, timeout):
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=str(cwd),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        elapsed = time.monotonic() - started
        return {
            "exit_code": completed.returncode,
            "stdout": completed.stdout,
            "stderr": completed.stderr,
            "timed_out": False,
            "elapsed_s": round(elapsed, 3),
        }
    except subprocess.TimeoutExpired as error:
        elapsed = time.monotonic() - started
        return {
            "exit_code": None,
            "stdout": error.stdout if isinstance(error.stdout, bytes) else b"",
            "stderr": error.stderr if isinstance(error.stderr, bytes) else b"",
            "timed_out": True,
            "elapsed_s": round(elapsed, 3),
        }
    except OSError as error:
        return {
            "exit_code": None,
            "stdout": b"",
            "stderr": str(error).encode("utf-8", "replace"),
            "timed_out": False,
            "elapsed_s": round(time.monotonic() - started, 3),
        }


def failure(case, message):
    return {"case": case, "ok": False, "message": message}


def run_case(binary, root, case_name, program, args, golden, timeout, keep):
    program_dir = root / "examples" / "programs"
    expected_path = (
        root / "crates" / "lsaca" / "tests" / "real_programs" / "expected" / golden
    )
    if not expected_path.is_file():
        return failure(case_name, "missing golden file: %s" % expected_path)
    expected = normalize(expected_path.read_bytes())

    command = [str(binary), "run", program + ".ls"]
    if args:
        command.append("--")
        command.extend(args)
    result = run_command(command, program_dir, timeout)

    run_dir = root / "target" / "real-programs" / "runs" / case_name
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "stdout.bin").write_bytes(result["stdout"])
    (run_dir / "stderr.bin").write_bytes(result["stderr"])
    (run_dir / "command.txt").write_text(
        " ".join(command) + "\n", encoding="utf-8"
    )

    problems = []
    if result["timed_out"]:
        problems.append("timed out after %.1f s" % timeout)
    elif result["exit_code"] != 0:
        problems.append("exit code %s" % result["exit_code"])
    stderr = result["stderr"].decode("utf-8", "replace")
    if stderr:
        problems.append("stderr was not empty: %s" % stderr.strip()[:2000])
    actual = normalize(result["stdout"])
    mismatch = describe_mismatch(actual, expected)
    if mismatch:
        problems.append(mismatch)
    elif actual != expected:
        problems.append("stdout differs after CRLF normalization")

    record = {
        "case": case_name,
        "command": command,
        "cwd": str(program_dir),
        "exit_code": result["exit_code"],
        "elapsed_s": result["elapsed_s"],
        "expected_bytes": len(expected),
        "actual_bytes": len(actual),
        "stderr": stderr,
        "ok": not problems,
        "run_dir": str(run_dir),
    }
    if problems:
        record["message"] = "; ".join(problems)
        return record
    if not keep:
        shutil.rmtree(run_dir, ignore_errors=True)
    return record


def compile_c_reference(root, gcc, program, timeout):
    bin_dir = root / "target" / "real-programs" / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    suffix = ".exe" if os.name == "nt" else ""
    executable = bin_dir / (program + "_c" + suffix)
    source = root / "examples" / "programs" / (program + ".c")
    command = [
        gcc,
        "-std=c17",
        "-O2",
        "-Wall",
        "-Wextra",
        "-Werror",
        str(source),
        "-lm",
        "-o",
        str(executable),
    ]
    try:
        completed = subprocess.run(
            command,
            cwd=str(root),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return None, "C compilation of %s timed out after %.1f s" % (program, timeout)
    except OSError as error:
        return None, "could not run gcc: %s" % error
    if completed.returncode != 0:
        details = completed.stderr.decode("utf-8", "replace").strip()
        return None, "C compilation of %s failed: %s" % (program, details[:4000])
    return executable, None


def run_c_case(root, executable, case_name, args, golden, timeout, keep):
    program_dir = root / "examples" / "programs"
    expected_path = (
        root / "crates" / "lsaca" / "tests" / "real_programs" / "expected" / golden
    )
    expected = normalize(expected_path.read_bytes())
    command = [str(executable)] + args
    result = run_command(command, program_dir, timeout)

    run_dir = root / "target" / "real-programs" / "runs" / ("c_" + case_name)
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "stdout.bin").write_bytes(result["stdout"])
    (run_dir / "stderr.bin").write_bytes(result["stderr"])
    (run_dir / "command.txt").write_text(
        " ".join(command) + "\n", encoding="utf-8"
    )

    problems = []
    if result["timed_out"]:
        problems.append("timed out after %.1f s" % timeout)
    elif result["exit_code"] != 0:
        problems.append("exit code %s" % result["exit_code"])
    stderr = result["stderr"].decode("utf-8", "replace")
    if stderr:
        problems.append("stderr was not empty: %s" % stderr.strip()[:2000])
    actual = normalize(result["stdout"])
    mismatch = describe_mismatch(actual, expected)
    if mismatch:
        problems.append(mismatch)

    record = {
        "case": "c_" + case_name,
        "command": command,
        "cwd": str(program_dir),
        "exit_code": result["exit_code"],
        "elapsed_s": result["elapsed_s"],
        "expected_bytes": len(expected),
        "actual_bytes": len(actual),
        "stderr": stderr,
        "ok": not problems,
        "run_dir": str(run_dir),
    }
    if problems:
        record["message"] = "; ".join(problems)
    elif not keep:
        shutil.rmtree(run_dir, ignore_errors=True)
    return record


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Compare the real example programs with their golden output."
    )
    parser.add_argument("--bin", required=True, help="path to the lsaca binary")
    parser.add_argument(
        "--root",
        default=str(Path(__file__).resolve().parent.parent),
        help="repository root (default: the checkout containing this script)",
    )
    parser.add_argument(
        "--no-c", action="store_true", help="skip compiling and running the C references"
    )
    parser.add_argument("--json", metavar="PATH", help="write a JSON report to PATH")
    parser.add_argument(
        "--timeout",
        type=float,
        default=120.0,
        metavar="SECONDS",
        help="per-command timeout in seconds (default: 120)",
    )
    parser.add_argument(
        "--keep",
        action="store_true",
        help="keep the per-case stdout/stderr under target/real-programs/runs",
    )
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    binary = Path(args.bin)
    if not binary.is_absolute():
        binary = Path.cwd() / binary
    if not binary.is_file() and os.name == "nt" and binary.suffix == "":
        binary = binary.with_name(binary.name + ".exe")
    if not binary.is_file():
        print("real programs: binary not found: %s" % binary)
        return 1

    gcc = None
    if not args.no_c:
        gcc = find_gcc()
        if not gcc:
            print(
                "real programs: gcc not found on PATH or at C:\\msys64\\ucrt64\\bin\\gcc.exe"
            )
            return 1

    results = []
    failures = []
    for case_name, program, case_args, golden in CASES:
        record = run_case(
            binary, root, case_name, program, case_args, golden, args.timeout, args.keep
        )
        results.append(record)
        if not record["ok"]:
            failures.append(record)

    c_executables = {}
    if not args.no_c:
        for program in PROGRAMS:
            executable, error = compile_c_reference(root, gcc, program, args.timeout)
            if error:
                failures.append({"case": "c_" + program, "ok": False, "message": error})
            else:
                c_executables[program] = executable
        for case_name, program, case_args, golden in CASES:
            executable = c_executables.get(program)
            if executable is None:
                continue
            record = run_c_case(
                root, executable, case_name, case_args, golden, args.timeout, args.keep
            )
            results.append(record)
            if not record["ok"]:
                failures.append(record)

    passed = sum(1 for record in results if record["ok"])
    total = len(CASES) + (len(CASES) if not args.no_c else 0)
    c_comparisons = len(results) - len(CASES)

    if args.json:
        destination = Path(args.json)
        if destination.parent and not destination.parent.exists():
            destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(
            json.dumps(
                {
                    "root": str(root),
                    "binary": str(binary),
                    "gcc": gcc,
                    "no_c": args.no_c,
                    "passed": passed,
                    "total": total,
                    "c_comparisons": c_comparisons,
                    "ok": not failures,
                    "results": results,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )

    if failures:
        for record in failures:
            print("FAIL %s: %s" % (record["case"], record.get("message", "failed")))
        print(
            "real programs: %d/%d cases passed (%d C comparisons)"
            % (passed, total, c_comparisons)
        )
        return 1

    print(
        "real programs: %d/%d cases passed (%d C comparisons)"
        % (passed, total, c_comparisons)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
