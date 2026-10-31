#!/usr/bin/env python3
"""Run every runnable lsaca example in docs/ against a real compiler binary.

Each fenced code block whose info string is exactly "ls" or starts with "ls,"
is a runnable example. Examples are written to a scratch work directory and
numbered in document order per page (example-01.ls, example-02.ls, ...), so
diagnostics and panic messages in the documentation line up with the files
that are executed.

Recognised info-string markers:

    ls                      compile in debug mode, run, expect exit 0
    ls,compile-fail         run "check", expect a non-zero exit and the printed diagnostic
    ls,panic                run, expect exit 101 and the printed panic message
    ls,release              compile with -r, run, expect exit 0
    ls,args=a,b             pass "a" and "b" to the program after "--"
    ls,stdin=a|b|c          feed a newline-separated stream on stdin
    ls,exit=N               expect exit code N (normal run)

The expected output is the text fenced block that follows the example,
optionally preceded by a one-line label (Output:, Error: or Panic:). A fenced
text block immediately after the example (no label) is also accepted.
Blank lines are ignored at both ends of the comparison; lines that a debug
build prints to stderr and that start with "DEBUG " are dropped.

Exit status: 0 when every example matches, 1 when any example fails.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DOCS = REPO_ROOT / "docs"
DEFAULT_WORK = REPO_ROOT / "target" / "docs-tmp" / "work"
DEFAULT_TMP = REPO_ROOT / "target" / "docs-tmp" / "tmp"
BIN_CANDIDATES = (
    REPO_ROOT / "target" / "release" / "lsaca.exe",
    REPO_ROOT / "target" / "debug" / "lsaca.exe",
    REPO_ROOT / "target" / "v16" / "bin" / "lsaca.exe",
)

BT = chr(96)
FENCE_RE = re.compile(r"^\s*(" + BT + r"{3,})(.*)$")
CLOSE_RE = re.compile(r"^\s*" + BT + r"{3,}\s*$")
LABEL_RE = re.compile(r"^(Output|Error|Panic):\s*$")
TEXT_INFOS = ("", "text")


def split_lines(text):
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return text.split("\n")


def parse_fences(lines):
    """Return every fenced block: dict(info, start, end, body, closed)."""
    fences = []
    i = 0
    count = len(lines)
    while i < count:
        match = FENCE_RE.match(lines[i])
        if not match:
            i += 1
            continue
        ticks = len(match.group(1))
        info = match.group(2).strip()
        start = i
        i += 1
        body = []
        closed = False
        while i < count:
            close = CLOSE_RE.match(lines[i])
            if close and len(lines[i].strip()) >= ticks:
                closed = True
                break
            body.append(lines[i])
            i += 1
        fences.append({"info": info, "start": start, "end": i, "body": body, "closed": closed})
        i += 1
    return fences


def parse_markers(info):
    """Parse an info string into an example descriptor, or None if not runnable."""
    parts = [part.strip() for part in info.split(",")]
    if not parts or parts[0] != "ls":
        return None
    desc = {
        "compile_fail": False,
        "panic": False,
        "release": False,
        "args": None,
        "stdin": None,
        "exit": None,
        "unknown": [],
    }
    for index in range(1, len(parts)):
        part = parts[index]
        if part == "":
            continue
        if part == "compile-fail":
            desc["compile_fail"] = True
        elif part == "panic":
            desc["panic"] = True
        elif part == "release":
            desc["release"] = True
        elif part.startswith("args="):
            desc["args"] = ",".join([part[len("args="):]] + parts[index + 1:]).strip(",")
            break
        elif part.startswith("stdin="):
            desc["stdin"] = part[len("stdin="):]
        elif part.startswith("exit="):
            try:
                desc["exit"] = int(part[len("exit="):])
            except ValueError:
                desc["unknown"].append(part)
        else:
            desc["unknown"].append(part)
    return desc


def expected_after(lines, fences_by_start, example):
    """Find the expected output block after an example fence."""
    index = example["end"] + 1
    count = len(lines)
    while index < count and lines[index].strip() == "":
        index += 1
    label = None
    if index < count:
        label_match = LABEL_RE.match(lines[index].strip())
        if label_match:
            label = label_match.group(1)
            index += 1
            while index < count and lines[index].strip() == "":
                index += 1
    block = fences_by_start.get(index)
    if block is None or block["info"] not in TEXT_INFOS:
        return None, label
    return "\n".join(block["body"]), label


def normalize(text, strip_debug_lines=False):
    lines = split_lines(text)
    if strip_debug_lines:
        lines = [line for line in lines if not line.startswith("DEBUG ")]
    result = [line.rstrip() for line in lines]
    while result and result[0] == "":
        result.pop(0)
    while result and result[-1] == "":
        result.pop()
    return "\n".join(result)


def build_command(binary, name, desc):
    if desc["compile_fail"]:
        return [binary, "check", name]
    command = [binary, "run"]
    if desc["release"]:
        command.append("-r")
    command.append(name)
    if desc["args"]:
        command.append("--")
        command.extend([part for part in desc["args"].split(",") if part])
    return command


def run_example(binary, name, desc, workdir, env, timeout):
    stdin_bytes = None
    if desc["stdin"] is not None:
        stdin_bytes = desc["stdin"].replace("|", "\n").encode("utf-8")
    command = build_command(binary, name, desc)
    try:
        completed = subprocess.run(
            command,
            cwd=str(workdir),
            env=env,
            input=stdin_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return {"timeout": True, "command": command}
    stdout = completed.stdout.decode("utf-8", "replace")
    stderr = completed.stderr.decode("utf-8", "replace")
    return {
        "timeout": False,
        "command": command,
        "exit": completed.returncode,
        "stdout": stdout,
        "stderr": stderr,
    }


def expected_exit(desc):
    if desc["panic"]:
        return 101
    if desc["compile_fail"]:
        return None
    return desc["exit"] if desc["exit"] is not None else 0


def describe_failure(rel, example_no, want_exit_text, expected_text, actual_exit, actual_text, command):
    lines = []
    lines.append("FAIL " + rel + " example-" + example_no + ".ls")
    lines.append("  command: " + " ".join(command))
    lines.append("  exit: got " + str(actual_exit) + ", expected " + str(want_exit_text))
    if expected_text != actual_text:
        expected_lines = expected_text.split("\n")
        actual_lines = actual_text.split("\n")
        limit = max(len(expected_lines), len(actual_lines))
        for index in range(limit):
            want = expected_lines[index] if index < len(expected_lines) else "<missing>"
            got = actual_lines[index] if index < len(actual_lines) else "<missing>"
            if want != got:
                lines.append("  first difference at line " + str(index + 1))
                lines.append("    expected: " + repr(want))
                lines.append("    actual:   " + repr(got))
                break
    if expected_text != actual_text:
        lines.append("  expected:")
        for line in expected_text.split("\n")[:40]:
            lines.append("    | " + line)
        lines.append("  actual:")
        for line in actual_text.split("\n")[:40]:
            lines.append("    | " + line)
    return "\n".join(lines)


def collect_pages(page_filter):
    pages = []
    readme = DOCS / "README.md"
    if readme.is_file():
        pages.append(readme)
    language = DOCS / "language"
    if language.is_dir():
        pages.extend(sorted(language.glob("*.md")))
    if page_filter:
        pages = [page for page in pages if page_filter in page.relative_to(REPO_ROOT).as_posix()]
    return pages


def main(argv):
    parser = argparse.ArgumentParser(description="Verify doc examples against the lsaca compiler.")
    parser.add_argument("--bin", default=os.environ.get("LSACA_BIN"), help="lsaca compiler binary to test")
    parser.add_argument("--page", default=None, help="only check pages whose path contains this substring")
    parser.add_argument("--work", default=str(DEFAULT_WORK), help="scratch directory")
    parser.add_argument("--timeout", type=float, default=60.0, help="per-example timeout in seconds")
    parser.add_argument("--keep", action="store_true", help="keep the scratch work directory")
    parser.add_argument("--verbose", action="store_true", help="print every example result")
    parser.add_argument("--max-report", type=int, default=25, help="maximum failures described in detail")
    args = parser.parse_args(argv)

    binary = args.bin
    if binary:
        binary_path = Path(binary)
        if not binary_path.is_absolute():
            binary_path = (REPO_ROOT / binary_path).resolve()
    else:
        binary_path = None
        for candidate in BIN_CANDIDATES:
            if candidate.is_file():
                binary_path = candidate
                break
        if binary_path is None:
            print("docs_verify: no lsaca binary found; pass --bin or set LSACA_BIN", file=sys.stderr)
            return 2
    if not binary_path.is_file():
        print("docs_verify: lsaca binary not found: " + str(binary_path), file=sys.stderr)
        return 2

    work_root = Path(args.work)
    if not work_root.is_absolute():
        work_root = REPO_ROOT / work_root
    if work_root.exists() and not args.keep:
        shutil.rmtree(work_root, ignore_errors=True)
    work_root.mkdir(parents=True, exist_ok=True)
    tmp_root = DEFAULT_TMP
    tmp_root.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["TEMP"] = str(tmp_root)
    env["TMP"] = str(tmp_root)
    env["TMPDIR"] = str(tmp_root)

    try:
        version = subprocess.run(
            [str(binary_path), "--version"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
        ).stdout.decode("utf-8", "replace").strip()
    except Exception as exc:
        version = "unknown (" + str(exc) + ")"
    print("docs_verify: binary " + str(binary_path))
    print("docs_verify: version " + version)

    total_checked = 0
    total_passed = 0
    total_failed = 0
    total_skipped = 0
    failures = []
    pages = collect_pages(args.page)
    for page in pages:
        rel = page.relative_to(REPO_ROOT).as_posix()
        lines = split_lines(page.read_text(encoding="utf-8"))
        fences = parse_fences(lines)
        fences_by_start = {fence["start"]: fence for fence in fences}
        page_dir = work_root / rel.replace("/", "__")
        page_dir.mkdir(parents=True, exist_ok=True)
        env["LSACA_CACHE_DIR"] = str(page_dir / ".cache")
        checked = 0
        passed = 0
        failed = 0
        skipped = 0
        example_no = 0
        for fence in fences:
            desc = parse_markers(fence["info"])
            if desc is None:
                continue
            example_no += 1
            checked += 1
            total_checked += 1
            name = "example-" + str(example_no).zfill(2) + ".ls"
            source = "\n".join(fence["body"]) + "\n"
            (page_dir / name).write_text(source, encoding="utf-8", newline="\n")
            expected_text, label = expected_after(lines, fences_by_start, fence)
            if desc["unknown"]:
                print("SKIP " + rel + " " + name + ": unknown marker(s) " + ", ".join(desc["unknown"]))
                skipped += 1
                total_skipped += 1
                continue
            if expected_text is None:
                failures.append("FAIL " + rel + " " + name + ": no expected Output:/Error:/Panic: text block found")
                failed += 1
                total_failed += 1
                continue
            result = run_example(str(binary_path), name, desc, page_dir, env, args.timeout)
            if result["timeout"]:
                failures.append("FAIL " + rel + " " + name + ": timed out after " + str(args.timeout) + " s")
                failed += 1
                total_failed += 1
                continue
            actual_exit = result["exit"]
            want_exit = expected_exit(desc)
            if desc["compile_fail"]:
                exit_ok = actual_exit != 0
                want_exit_text = "non-zero"
            else:
                exit_ok = actual_exit == want_exit
                want_exit_text = str(want_exit)
            if desc["compile_fail"] or desc["panic"]:
                actual_text = result["stderr"] + result["stdout"]
            else:
                actual_text = result["stdout"]
            actual_clean = normalize(actual_text, strip_debug_lines=True)
            expected_clean = normalize(expected_text)
            if exit_ok and actual_clean == expected_clean:
                passed += 1
                total_passed += 1
                if args.verbose:
                    print("PASS " + rel + " " + name)
            else:
                failed += 1
                total_failed += 1
                failures.append(
                    describe_failure(
                        rel,
                        str(example_no).zfill(2),
                        want_exit_text,
                        expected_clean,
                        actual_exit,
                        actual_clean,
                        result["command"],
                    )
                )
        if checked:
            print("PAGE " + rel + " checked=" + str(checked) + " passed=" + str(passed) +
                  " failed=" + str(failed) + " skipped=" + str(skipped))
        elif args.verbose:
            print("PAGE " + rel + " checked=0 (no runnable examples)")

    printed = 0
    for failure in failures:
        if printed >= args.max_report:
            print("... " + str(len(failures) - printed) + " more failure(s) not shown (use --max-report)")
            break
        print(failure)
        printed += 1

    print("TOTAL checked=" + str(total_checked) + " passed=" + str(total_passed) +
          " failed=" + str(total_failed) + " skipped=" + str(total_skipped))
    return 0 if total_failed == 0 else 1


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except KeyboardInterrupt:
        sys.exit(130)
