#!/usr/bin/env python3
"""Check docs/language/21-benchmarks.md against the official benchmark results.

The page must mirror the measured data exactly:

  * the compile-time table, the runtime table and the peak-memory table must be
    cell-for-cell identical to the same tables in benchmarks/results/latest.md;
  * the ratio table must be recomputable from the medians stored in
    benchmarks/results/latest.json;
  * every runtime and memory cell of latest.md must be recomputable from the
    medians / peaks in latest.json;
  * the environment table and the workload checksum table on the page must
    agree with latest.json and latest.md;
  * headline numbers quoted in the page prose must be derivable from the data
    (cold-build range, cached-build range, compile_large figures).

Exit status: 0 when everything agrees, 1 otherwise.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PAGE = REPO_ROOT / "docs" / "language" / "21-benchmarks.md"
LATEST_MD = REPO_ROOT / "benchmarks" / "results" / "latest.md"
LATEST_JSON = REPO_ROOT / "benchmarks" / "results" / "latest.json"

BT = chr(96)
FENCE_RE = re.compile(r"^\s*" + BT + r"{3,}")
HEADING_RE = re.compile(r"^#{1,6}\s+(.*?)\s*#*\s*$")
SEPARATOR_RE = re.compile(r"^:?-{2,}:?$")
INT_RE = re.compile(r"^\d+$")
NUM_RE = re.compile(r"^\d+(\.\d+)?$")
RATIO_RE = re.compile(r"^\d+\.\d+x$")
CHECKSUM_RE = re.compile(r"checksum=([0-9][0-9,]*)")

LANG_KEY = {
    "lsaca": "lsaca",
    "C (gcc -O2)": "c",
    "Rust (rustc -O)": "rust",
    "Python 3": "python",
    "Node.js": "node",
}

# The ratio table labels are shorter than the runtime table labels.
RATIO_KEY = {
    "lsaca": "lsaca",
    "Rust": "rust",
    "Python 3": "python",
    "Node.js": "node",
}


def lines_of(text):
    return text.replace("\r\n", "\n").replace("\r", "\n").split("\n")


def cells(line):
    stripped = line.strip()
    if stripped.startswith("|"):
        stripped = stripped[1:]
    if stripped.endswith("|"):
        stripped = stripped[:-1]
    return [cell.strip() for cell in stripped.split("|")]


def is_separator(line):
    if not line.strip().startswith("|"):
        return False
    parts = cells(line)
    return bool(parts) and all(SEPARATOR_RE.match(part) for part in parts)


def parse_tables(text):
    lines = lines_of(text)
    tables = []
    heading = ""
    in_fence = False
    index = 0
    while index < len(lines):
        line = lines[index]
        if FENCE_RE.match(line):
            in_fence = not in_fence
            index += 1
            continue
        if in_fence:
            index += 1
            continue
        heading_match = HEADING_RE.match(line)
        if heading_match:
            heading = heading_match.group(1).strip()
            index += 1
            continue
        if line.strip().startswith("|") and index + 1 < len(lines) and is_separator(lines[index + 1]):
            header = cells(line)
            rows = []
            cursor = index + 2
            while cursor < len(lines) and lines[cursor].strip().startswith("|"):
                rows.append(cells(lines[cursor]))
                cursor += 1
            tables.append({"heading": heading, "header": header, "rows": rows, "line": index + 1})
            index = cursor
            continue
        index += 1
    return tables


def header_of(table):
    return [cell.strip() for cell in table["header"]]


def is_language_table(table):
    header = header_of(table)
    return header[:1] == ["language"] and "fib" in header and "iterator_pipeline" in header


def classify(table):
    if not is_language_table(table):
        return None
    if not table["rows"]:
        return None
    sample = table["rows"][0][1] if len(table["rows"][0]) > 1 else ""
    if "(" in sample:
        return "runtime"
    if sample.endswith("x"):
        return "ratio"
    if NUM_RE.match(sample):
        return "memory"
    return None


def find_table(tables, kind):
    for table in tables:
        if classify(table) == kind:
            return table
    return None


def find_heading_table(tables, keyword):
    for table in tables:
        if keyword in table["heading"]:
            return table
    return None


def fmt_ms(seconds):
    return "{0:.1f}".format(seconds * 1000.0)


def fmt_fixed(value, digits):
    return ("{0:." + str(digits) + "f}").format(value)


def plain_program(label):
    """Strip the '(compile-only)' suffix the tables use for compile-only rows."""
    return re.sub(r"\s*\(compile-only\)$", "", label)


def fmt_int(value):
    if abs(value - round(value)) < 0.049:
        return str(int(round(value)))
    return fmt_ms(value / 1000.0)


class Checker:
    def __init__(self):
        self.failures = []
        self.checks = 0

    def check(self, condition, message):
        self.checks += 1
        if not condition:
            self.failures.append(message)

    def equal(self, actual, expected, message):
        self.checks += 1
        if actual != expected:
            self.failures.append(message + ": expected " + repr(expected) + ", found " + repr(actual))

    def tables_equal(self, label, page_table, latest_table):
        if page_table is None or latest_table is None:
            self.failures.append(label + ": table missing on page or in latest.md")
            return
        self.equal(header_of(page_table), header_of(latest_table), label + " header")
        if len(page_table["rows"]) != len(latest_table["rows"]):
            self.failures.append(label + ": row count " + str(len(page_table["rows"])) +
                                 " on page, " + str(len(latest_table["rows"])) + " in latest.md")
            return
        for page_row, latest_row in zip(page_table["rows"], latest_table["rows"]):
            self.equal(page_row, latest_row, label + " row for " + (page_row[0] if page_row else "?"))


def main():
    page_text = PAGE.read_text(encoding="utf-8")
    latest_text = LATEST_MD.read_text(encoding="utf-8")
    data = json.loads(LATEST_JSON.read_text(encoding="utf-8"))

    checker = Checker()
    page_tables = parse_tables(page_text)
    latest_tables = parse_tables(latest_text)

    page_compile = find_heading_table(page_tables, "Compile time")
    latest_compile = find_heading_table(latest_tables, "Compile time")
    page_runtime = find_table(page_tables, "runtime")
    latest_runtime = find_table(latest_tables, "runtime")
    page_memory = find_table(page_tables, "memory")
    latest_memory = find_table(latest_tables, "memory")
    page_ratio = find_table(page_tables, "ratio")

    checker.tables_equal("compile table", page_compile, latest_compile)
    checker.tables_equal("runtime table", page_runtime, latest_runtime)
    checker.tables_equal("memory table", page_memory, latest_memory)

    if page_compile is None or latest_compile is None or page_runtime is None or latest_runtime is None:
        print("check_bench21: cannot locate the benchmark tables")
        return 1

    programs = list(data["settings"]["programs"])
    compile_only = set(data["settings"].get("compile_only_programs", []))
    runtime_programs = [name for name in programs if name not in compile_only]
    runtime_header = header_of(latest_runtime)
    table_programs = runtime_header[1:]
    checker.equal(table_programs, runtime_programs, "runtime workload order")

    # latest.md runtime and memory cells must be recomputable from latest.json.
    latest_rows = {row[0]: row[1:] for row in latest_runtime["rows"]}
    for language_label, language_key in LANG_KEY.items():
        row = latest_rows.get(language_label)
        if row is None:
            checker.failures.append("latest.md runtime table: missing row " + language_label)
            continue
        for name, cell in zip(runtime_programs, row):
            runtime = data["programs"][name]["languages"][language_key]["runtime"]
            expected = fmt_ms(runtime["median_s"]) + " (" + fmt_ms(runtime["min_s"]) + ")"
            checker.equal(cell, expected, "latest.md runtime " + language_label + " / " + name)

    memory_rows = {row[0]: row[1:] for row in latest_memory["rows"]}
    for language_label, language_key in LANG_KEY.items():
        row = memory_rows.get(language_label)
        if row is None:
            checker.failures.append("latest.md memory table: missing row " + language_label)
            continue
        for name, cell in zip(runtime_programs, row):
            peak = data["programs"][name]["languages"][language_key]["memory"]["peak_mib"]
            checker.equal(cell, fmt_fixed(peak, 1), "latest.md memory " + language_label + " / " + name)

    # Ratio table on the page must be recomputable from latest.json medians.
    if page_ratio is None:
        checker.failures.append("ratio table missing from docs/language/21-benchmarks.md")
    else:
        c_median = {}
        for name in runtime_programs:
            c_median[name] = data["programs"][name]["languages"]["c"]["runtime"]["median_s"]
        ratio_rows = {row[0]: row[1:] for row in page_ratio["rows"]}
        for language_label in ("lsaca", "Rust", "Python 3", "Node.js"):
            row = ratio_rows.get(language_label)
            if row is None:
                checker.failures.append("ratio table: missing row " + language_label)
                continue
            language_key = RATIO_KEY[language_label]
            for name, cell in zip(runtime_programs, row):
                median = data["programs"][name]["languages"][language_key]["runtime"]["median_s"]
                expected = "{0:.2f}x".format(median / c_median[name])
                checker.equal(cell, expected, "ratio " + language_label + " / " + name)

    # Compile table cells must be recomputable from latest.json as well.
    compile_rows = {}
    for row in latest_compile["rows"]:
        compile_rows.setdefault(plain_program(row[0]), row)
    for name in programs:
        row = compile_rows.get(name)
        if row is None:
            checker.failures.append("latest.md compile table: missing row " + name)
            continue
        lsaca_compile = data["programs"][name]["languages"]["lsaca"]["compile"]
        checker.equal(row[1], fmt_fixed(lsaca_compile["cold_s"], 3), "cold build " + name)
        checker.equal(row[2], fmt_fixed(lsaca_compile["cached_median_s"], 3), "cached build " + name)
        checker.equal(row[3], "n/a", "emit-c column " + name)
        if name in compile_only:
            checker.equal(row[4], "n/a", "gcc column " + name)
            checker.equal(row[5], "n/a", "rustc column " + name)
        else:
            gcc_compile = data["programs"][name]["languages"]["c"]["compile"]
            rust_compile = data["programs"][name]["languages"]["rust"]["compile"]
            checker.equal(row[4], fmt_fixed(gcc_compile["median_s"], 3), "gcc compile " + name)
            checker.equal(row[5], fmt_fixed(rust_compile["median_s"], 3), "rustc compile " + name)
        checker.equal(row[6], "n/a", "python compile " + name)
        checker.equal(row[7], "n/a", "node compile " + name)

    # Environment: JSON is authoritative; latest.md bullets and the page table must agree.
    toolchains = data["toolchains"]
    settings = data["settings"]
    version = toolchains["lsaca"].split()[-1]
    generated_at = data["generated_at"]
    generated_display = generated_at.replace("T", " ").replace("+", " +").replace(" 0", " 0")
    if "T" in generated_at:
        date_part, rest = generated_at.split("T", 1)
        offset = ""
        if "+" in rest:
            rest, offset = rest.split("+", 1)
            offset = " +" + offset
        elif "-" in rest[1:]:
            pass
        generated_display = date_part + " " + rest + offset
    mtime = toolchains["lsaca_compiler_mtime"]
    mtime_display = mtime.replace("T", " ")
    if "T" in mtime:
        date_part, rest = mtime.split("T", 1)
        offset = ""
        if "+" in rest:
            rest, offset = rest.split("+", 1)
            offset = " +" + offset
        mtime_display = date_part + " " + rest + offset

    page_env_table = find_heading_table(page_tables, "Environment")
    env = {}
    if page_env_table is None:
        checker.failures.append("environment table missing from docs/language/21-benchmarks.md")
    else:
        for row in page_env_table["rows"]:
            if len(row) >= 2:
                env[row[0]] = row[1]
        checker.check(generated_display in env.get("Measured at", ""), "environment: Measured at must contain " + generated_display)
        lsaca_row = env.get("lsaca", "")
        checker.check(version in lsaca_row, "environment: lsaca row must contain version " + version)
        checker.check(toolchains["repository_commit"] in lsaca_row, "environment: lsaca row must contain commit " + toolchains["repository_commit"])
        checker.check(toolchains["lsaca_compiler_sha256"][:16] in lsaca_row, "environment: lsaca row must contain sha256 prefix")
        checker.check(mtime_display in lsaca_row, "environment: lsaca row must contain compiler mtime " + mtime_display)
        checker.check("15.2.0" in env.get("C compiler", ""), "environment: C compiler row must contain gcc 15.2.0")
        checker.check("1.99.0" in env.get("Rust compiler", ""), "environment: Rust compiler row must contain rustc 1.99.0")
        checker.check("3.14.4" in env.get("Python", ""), "environment: Python row must contain 3.14.4")
        checker.check("v24.14.1" in env.get("Node.js", ""), "environment: Node.js row must contain v24.14.1")
        harness_row = env.get("Harness", "")
        checker.check(str(len(programs)) + " programs (" + str(len(runtime_programs)) + " executed)" in harness_row,
                      "environment: Harness row program counts")
        checker.check("1 warm-up + " + str(settings["runs"]) + " runs" in harness_row, "environment: Harness row run counts")
        checker.check(str(settings["compile_repeats"]) + " compile repeats" in harness_row, "environment: Harness row compile repeats")

    latest_lines = lines_of(latest_text)
    latest_env = {}
    in_env = False
    generated_line = ""
    for line in latest_lines:
        if line.strip() == "## Environment":
            in_env = True
            continue
        if in_env and line.startswith("## "):
            break
        if in_env:
            match = re.match(r"^- ([^:]+): (.*)$", line)
            if match:
                latest_env[match.group(1).strip()] = match.group(2).strip()
        if line.startswith("Generated:"):
            generated_line = line
    checker.check(generated_at in generated_line, "latest.md: Generated line must contain " + generated_at)
    checker.check(version in latest_env.get("lsaca", ""), "latest.md: lsaca environment line version " + version)
    checker.check("15.2.0" in latest_env.get("gcc", ""), "latest.md: gcc environment line")
    checker.check("1.99.0" in latest_env.get("rustc", ""), "latest.md: rustc environment line")
    checker.check("3.14.4" in latest_env.get("Python", ""), "latest.md: Python environment line")
    checker.check("v24.14.1" in latest_env.get("Node.js", ""), "latest.md: Node.js environment line")
    checker.check(latest_env.get("lsaca build mode", "") == "official", "latest.md: build mode must be official")

    # Workload checksum table on the page vs latest.md vs latest.json.
    latest_workloads = {}
    in_workloads = False
    for line in latest_lines:
        if line.strip() == "## Workloads":
            in_workloads = True
            continue
        if in_workloads and line.startswith("## "):
            break
        if in_workloads:
            match = re.match(r"^- ([a-z0-9_]+): (.*)$", line)
            if not match:
                continue
            checksum_match = CHECKSUM_RE.search(match.group(2))
            if checksum_match:
                latest_workloads[match.group(1)] = checksum_match.group(1).rstrip(",")
    page_workloads = {}
    page_workload_table = find_heading_table(page_tables, "Workloads")
    if page_workload_table is None:
        checker.failures.append("workload table missing from docs/language/21-benchmarks.md")
    else:
        for row in page_workload_table["rows"]:
            if len(row) < 2:
                continue
            name = row[0].replace(BT, "")
            checksum_match = CHECKSUM_RE.search(BT.join(row[1:]).replace(BT, ""))
            if checksum_match:
                page_workloads[name] = checksum_match.group(1).rstrip(",")
    checker.equal(sorted(page_workloads), sorted(programs), "page workload table names")
    checker.equal(sorted(latest_workloads), sorted(programs), "latest.md workload bullet names")
    for name in programs:
        checker.equal(page_workloads.get(name), latest_workloads.get(name), "workload checksum " + name)
        expected_match = CHECKSUM_RE.search(data["programs"][name]["expected"])
        expected = expected_match.group(1).rstrip(",") if expected_match else None
        checker.equal(page_workloads.get(name), expected, "json workload checksum " + name)

    # Derived headline numbers quoted in the page prose.
    cold_seconds = [row[1] for row in latest_compile["rows"] if plain_program(row[0]) not in compile_only]
    cached_seconds = [row[2] for row in latest_compile["rows"] if plain_program(row[0]) not in compile_only]
    if cold_seconds:
        cold_values = [float(value) for value in cold_seconds]
        cold_range = fmt_fixed(min(cold_values), 2) + "-" + fmt_fixed(max(cold_values), 2) + " s"
        checker.check(cold_range in page_text, "page prose: cold build range " + cold_range)
    if cached_seconds:
        cached_values = [float(value) * 1000.0 for value in cached_seconds]
        cached_range = fmt_int(min(cached_values) * 1000.0 / 1000.0) + "-" + fmt_int(max(cached_values) * 1000.0 / 1000.0) + " ms"
        checker.check(cached_range in page_text, "page prose: cached build range " + cached_range)
    large_row = compile_rows.get("compile_large")
    if large_row is not None:
        checker.check(fmt_fixed(float(large_row[1]), 2) + " s" in page_text, "page prose: compile_large cold build")
        checker.check(fmt_int(float(large_row[2]) * 1000.0 / 1000.0) in page_text, "page prose: compile_large cached build")
    large_compile = data["programs"]["compile_large"]["languages"]["lsaca"]["compile"]
    generated_c = large_compile["generated_c_bytes"]
    checker.check("{:,}".format(generated_c) in page_text, "page prose: generated C byte count " + "{:,}".format(generated_c))
    checker.check(fmt_fixed(generated_c / 1000000.0, 2) + " MB" in page_text, "page prose: generated C megabytes")
    checker.check(generated_at[:10] in page_text, "page prose: benchmark date " + generated_at[:10])
    checker.check(version in page_text, "page prose: lsaca version " + version)

    for failure in checker.failures:
        print("FAIL " + failure)
    print("check_bench21: " + str(checker.checks) + " checks, " + str(len(checker.failures)) + " failure(s)")
    if not checker.failures:
        print("check_bench21: page 21 tables are identical to latest.md and recomputable from latest.json")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
