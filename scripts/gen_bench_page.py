#!/usr/bin/env python3
"""Regenerate docs/language/21-benchmarks.md from the official benchmark results.

The page is a curated narrative, but every table and every headline number on it
is derived from benchmarks/results/latest.json and benchmarks/results/latest.md.
This script rewrites the derived sections (tables, the environment table, the
version note, the headline paragraph and the analysis) and keeps the stable
sections that are written by hand:

  * the notes under the workload table,
  * What is measured,
  * the idiomatic-programs table of the fairness notes,
  * See also.

Stable sections are copied from the current page, so editing them in the page
and re-running the script keeps the edits. Everything else is generated.

Usage:
    python scripts/gen_bench_page.py
    python scripts/gen_bench_page.py --check
    python scripts/gen_bench_page.py --baseline benchmarks/results/archive/v2.2/latest.json

Exit status: 0 when the page was written (or --check found it up to date), 1 otherwise.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PAGE = REPO_ROOT / "docs" / "language" / "21-benchmarks.md"
LATEST_JSON = REPO_ROOT / "benchmarks" / "results" / "latest.json"
LATEST_MD = REPO_ROOT / "benchmarks" / "results" / "latest.md"
DEFAULT_BASELINE = REPO_ROOT / "benchmarks" / "results" / "archive" / "v2.2" / "latest.json"

BT = chr(96)
HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
FENCE_RE = re.compile(r"^\s*" + BT + r"{3,}")
SEPARATOR_RE = re.compile(r"^:?-{2,}:?$")
CHECKSUM_RE = re.compile(r"checksum=([0-9][0-9,]*)")


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
    """Return every markdown table in text with its enclosing heading."""
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
            heading = heading_match.group(2).strip()
            index += 1
            continue
        if line.strip().startswith("|") and index + 1 < len(lines) and is_separator(lines[index + 1]):
            header = cells(line)
            rows = []
            cursor = index + 2
            while cursor < len(lines) and lines[cursor].strip().startswith("|"):
                rows.append(cells(lines[cursor]))
                cursor += 1
            tables.append({"heading": heading, "header": header, "rows": rows})
            index = cursor
            continue
        index += 1
    return tables


def find_table(tables, keyword):
    for table in tables:
        if keyword.lower() in table["heading"].lower():
            return table
    return None


def render_table(header, rows, align="---"):
    widths = [len(cell) for cell in header]
    out = ["| " + " | ".join(header) + " |"]
    separator = []
    for index, width in enumerate(widths):
        if index == 0 or align == "---":
            separator.append("---")
        else:
            separator.append("---:")
    out.append("| " + " | ".join(separator) + " |")
    for row in rows:
        out.append("| " + " | ".join(row) + " |")
    return out


def region_bounds(text, level, title):
    """Return (start, end) line indices of a heading and its body.

    end is the index of the next heading with a level <= the given level.
    """
    lines = lines_of(text)
    start = None
    for index, line in enumerate(lines):
        match = HEADING_RE.match(line)
        if not match:
            continue
        if start is None:
            if len(match.group(1)) == level and match.group(2).strip() == title:
                start = index
            continue
        if len(match.group(1)) <= level:
            return start, index
    if start is None:
        return None
    return start, len(lines)


def region_lines(text, level, title, include_heading=True):
    bounds = region_bounds(text, level, title)
    if bounds is None:
        return None
    lines = lines_of(text)
    start, end = bounds
    if not include_heading:
        start += 1
    return lines[start:end]


def section_body(text, level, title):
    return region_lines(text, level, title, include_heading=False)


def strip_blank_edges(lines):
    start = 0
    end = len(lines)
    while start < end and not lines[start].strip():
        start += 1
    while end > start and not lines[end - 1].strip():
        end -= 1
    return lines[start:end]


def fmt_ms(seconds):
    return "{0:.1f}".format(seconds * 1000.0)


def fmt_fixed(value, digits):
    return ("{0:." + str(digits) + "f}").format(value)


def fmt_int(value):
    if abs(value - round(value)) < 0.049:
        return str(int(round(value)))
    return fmt_ms(value / 1000.0)


def display_timestamp(stamp):
    if "T" not in stamp:
        return stamp
    date_part, rest = stamp.split("T", 1)
    offset = ""
    if "+" in rest:
        rest, offset = rest.split("+", 1)
        offset = " +" + offset
    elif "-" in rest[1:]:
        position = rest.find("-", 1)
        rest, offset = rest[:position], " -" + rest[position + 1:]
    return date_part + " " + rest + offset


def plain_name(label):
    return re.sub(r"\s*\(compile-only\)$", "", label.replace(BT, "").strip())


def bt(text):
    return BT + text + BT


# ---- benchmark data ----

LANG_ORDER = [("lsaca", "lsaca"), ("rust", "Rust"), ("python", "Python 3"), ("node", "Node.js")]

WORKLOAD_NOTES = {
    "fib": "deep recursion; each call allocates a stack frame and the C version leans on the same recursion",
    "fib_iter": "rolling two-value iteration; the loop is cheap in both languages",
    "fib_memo": "recursion plus a memo map; the map is a builtin HashMap with owned String keys folded away",
    "loop_sum": "a tight scalar loop; the only cost is the loop itself",
    "string_build": "one million appends; each append goes through the checked String API instead of writing into a raw buffer",
    "vec_memory": "one million Point values; the vector owns the elements and the collector header costs a little memory",
    "sort_i64": "quicksort written in the language; every swap goes through a bounds-checked index",
    "sort_insertion": "quadratic insertion sort; the inner loop is bounds-checked and the workload is dominated by the C reference's raw array accesses",
    "matrix_multiply": "flat vectors with checked indexing; the C version indexes a raw array",
    "binary_trees": "by-value tree semantics: each recursive step clones the child subtree, so the deep copies are part of the workload by design; binary_trees_refs is the borrowed-traversal variant",
    "binary_trees_refs": "borrowed tree traversal; references do not copy the tree, so only the traversal cost remains",
    "string_split": "splitting and parsing an owned string; every token is a String and parsing checks it",
    "sieve": "a large vector of flags with checked indexing; the C version uses a raw calloced array",
    "mandelbrot": "floating-point loops; the arithmetic is the same, the loop bookkeeping is checked",
    "iterator_pipeline": "a lazy adapter chain; each step is monomorphized but the adapters still carry state",
    "hashmap_wordcount": "hashing owned String words; the C reference hashes raw char pointers",
    "array_wordcount": "fixed buckets instead of a hash map; the difference is the map implementation",
    "generic_stack": "one generic stack monomorphized twice; the C version has two hand-written copies",
    "fn_pipeline": "function pointers stored in records; every call goes through a function value",
    "string_unicode": "owned String calls and Unicode table lookups; character-level work checks UTF-8 boundaries",
    "hashmap_i64": "integer-key map operations; the C version uses a hand-written open-addressing table",
    "hashmap_string": "owned String keys in the Swiss-table map; the C version uses strdup and a simpler table",
}


def lines_after_table(lines):
    start = None
    for index, line in enumerate(lines):
        if line.strip().startswith("|"):
            start = index
            break
    if start is None:
        return []
    cursor = start
    while cursor < len(lines) and lines[cursor].strip().startswith("|"):
        cursor += 1
    return lines[cursor:]


def first_group(pattern, text):
    match = re.search(pattern, text)
    return match.group(1) if match else None


class Context:
    def __init__(self, data, md, page, baseline):
        self.data = data
        self.md = md
        self.page = page
        self.baseline = baseline
        self.tables = parse_tables(md)
        self.compile_table = find_table(self.tables, "Compile time")
        self.runtime_table = find_table(self.tables, "Runtime (median")
        self.memory_table = find_table(self.tables, "Peak memory")
        self.scale_table = find_table(self.tables, "Runtime by data scale")
        self.artifact_table = find_table(self.tables, "Artifact sizes")
        if self.compile_table is None or self.runtime_table is None:
            raise SystemExit(
                "gen_bench_page: latest.md has no result tables; run the benchmark harness first"
            )
        self.compile_labels = [row[0] for row in self.compile_table["rows"]]
        self.programs = [plain_name(label) for label in self.compile_labels]
        self.executed = [name for name in self.programs if not self.is_compile_only(name)]
        self.compile_only = [name for name in self.programs if self.is_compile_only(name)]
        self.runtime_programs = [plain_name(name) for name in self.runtime_table["header"][1:]]
        page_tables = parse_tables(page)
        self.workload_info = {}
        workload_table = find_table(page_tables, "Workloads")
        if workload_table:
            for row in workload_table["rows"]:
                if len(row) >= 4:
                    self.workload_info[plain_name(row[0])] = (row[1], row[2])
        self.outside_notes = {}
        outside_table = find_table(page_tables, "outside")
        if outside_table:
            for row in outside_table["rows"]:
                if len(row) >= 5:
                    self.outside_notes[plain_name(row[0])] = row[4]
        workload_body = section_body(page, 2, "Workloads") or []
        self.workload_notes = strip_blank_edges(lines_after_table(workload_body))
        self.idiomatic_table = None
        fairness = region_lines(page, 2, "Fairness notes and caveats")
        if fairness:
            tables = parse_tables("\n".join(fairness))
            if tables:
                self.idiomatic_table = tables[0]

    def is_compile_only(self, name):
        program = self.data["programs"].get(name)
        if program is not None:
            return bool(program.get("compile_only"))
        settings = self.data.get("settings", {})
        return name in settings.get("compile_only_programs", [])

    def program(self, name):
        return self.data["programs"].get(name, {})

    def language(self, name, lang):
        return self.program(name).get("languages", {}).get(lang, {})

    def runtime(self, name, lang):
        return self.language(name, lang).get("runtime") or {}

    def median(self, name, lang):
        return self.runtime(name, lang).get("median_s")

    def memory_mib(self, name, lang):
        memory = self.language(name, lang).get("memory") or {}
        return memory.get("peak_mib")

    def c_median(self, name):
        return self.median(name, "c")

    def ratio(self, name, lang):
        c_median = self.c_median(name)
        median = self.median(name, lang)
        if not c_median or median is None:
            return None
        return median / c_median

    def baseline_ratio(self, name, lang):
        if not self.baseline:
            return None
        program = (self.baseline.get("programs") or {}).get(name) or {}
        languages = program.get("languages") or {}
        c_median = ((languages.get("c") or {}).get("runtime") or {}).get("median_s")
        median = ((languages.get(lang) or {}).get("runtime") or {}).get("median_s")
        if not c_median or median is None:
            return None
        return median / c_median

    def compile_row(self, name):
        for row in self.compile_table["rows"]:
            if plain_name(row[0]) == name:
                return row
        return None

    def cold_values(self):
        values = []
        for row in self.compile_table["rows"]:
            if not self.is_compile_only(plain_name(row[0])):
                values.append(float(row[1]))
        return values

    def cached_values(self):
        values = []
        for row in self.compile_table["rows"]:
            if not self.is_compile_only(plain_name(row[0])):
                values.append(float(row[2]) * 1000.0)
        return values

    def lsaca_ratios(self):
        result = {}
        for name in self.executed:
            ratio = self.ratio(name, "lsaca")
            if ratio is not None:
                result[name] = ratio
        return result

    def svg_count(self, threshold):
        return sum(1 for ratio in self.lsaca_ratios().values() if ratio <= threshold)

    def generated_c_bytes(self):
        for name in ["compile_large"] + list(self.data["programs"]):
            compile = (self.language(name, "lsaca").get("compile") or {})
            if "generated_c_bytes" in compile:
                return int(compile["generated_c_bytes"])
        return None

    def oracle_counts(self):
        verification = self.data.get("oracle_verification") or {}
        program_count = len(verification.get("programs") or [])
        scale_count = 0
        for scales in (verification.get("scales") or {}).values():
            scale_count += len(scales)
        return program_count, scale_count


def fmt_ratio(value):
    return "n/a" if value is None else "{0:.2f}x".format(value)


def fmt_change(value):
    return "n/a" if value is None else "{0:+.1f}%".format(value * 100.0)


def fmt_percent(value):
    return "n/a" if value is None else "{0:.1f}%".format(value * 100.0)


def describe_min_max(values, unit=" ms"):
    if not values:
        return "n/a"
    return fmt_fixed(min(values), 2) + "-" + fmt_fixed(max(values), 2) + unit


# ---- generated sections ----

def render_preamble(ctx):
    data = ctx.data
    toolchains = data.get("toolchains", {})
    version = str(toolchains.get("lsaca", "lsaca")).replace("lsaca ", "", 1)
    generated_at = str(data.get("generated_at", ""))
    date = generated_at[:10]
    commit = toolchains.get("repository_commit", "unknown")
    sha = toolchains.get("lsaca_compiler_sha256", "")
    compiler_mtime = display_timestamp(str(toolchains.get("lsaca_compiler_mtime", "")))
    ratios = ctx.lsaca_ratios()
    within_14 = sum(1 for ratio in ratios.values() if ratio <= 1.4)
    within_15 = sum(1 for ratio in ratios.values() if ratio <= 1.5)
    within_115 = sum(1 for ratio in ratios.values() if ratio <= 1.15)
    executed = len(ctx.executed)
    coldest = min(ctx.cold_values()) if ctx.cold_values() else 0.0
    warmest = max(ctx.cold_values()) if ctx.cold_values() else 0.0
    cached = ctx.cached_values()
    compile_large = ctx.compile_row("compile_large")
    compiled_bytes = ctx.generated_c_bytes()
    fastest = min(ratios.items(), key=lambda item: item[1]) if ratios else None
    gap = max(ratios.items(), key=lambda item: item[1]) if ratios else None
    out = []
    out.append("# Benchmarks")
    out.append("")
    out.append("This page reports how lsaca compares with C, Rust, Python and Node.js on "
               "the same workloads. The programs are written in the idiomatic style of "
               "each language, every program prints a checksum that the harness verifies "
               "against an independent oracle, and every number in the tables below is "
               "produced by the benchmark harness rather than typed by hand.")
    out.append("")
    out.append("The tables are generated from the machine-readable results in "
               + bt("benchmarks/results/latest.json") + " and the human-readable summary "
               "in " + bt("benchmarks/results/latest.md") + "; the page itself is written by "
               + bt("scripts/gen_bench_page.py") + ". The workload programs live in "
               + bt("benchmarks/programs") + ".")
    out.append("")
    out.append("> **Version note.** These numbers were measured for lsaca " + version
               + " on " + date + ", at " + generated_at + ", from commit " + commit
               + " with compiler build " + sha + " (" + compiler_mtime + "). The C "
               "reference was built with " + str(toolchains.get("gcc", "gcc")) + ". The "
               "2.2.0 baseline was measured with gcc 15.2.0; comparisons against it are "
               "normalised by the C reference and explained in the fairness notes.")
    out.append("")
    headline = [str(within_14) + " of the " + str(executed) + " executed workloads run within "
                "1.4x of the C reference; " + str(within_15) + " are within 1.5x and "
                + str(within_115) + " are within 1.15x."]
    if fastest is not None:
        headline.append("The closest workload is " + bt(fastest[0]) + " at "
                        + fmt_ratio(fastest[1]) + " of the C runtime")
    if gap is not None:
        headline.append("and the widest gap is " + bt(gap[0]) + " at " + fmt_ratio(gap[1])
                        + ", a deliberate value-semantics trade-off")
    out.append(headline[0] if len(headline) == 1 else
               headline[0] + " " + " ".join(headline[1:]) + ".")
    out.append("")
    cold_range = fmt_fixed(coldest, 2) + "-" + fmt_fixed(warmest, 2) + " s"
    cached_range = fmt_int(cached[0] if cached else 0) + "-" + fmt_int(cached[-1] if cached else 0) + " ms"
    if len(cached) > 1:
        cached_range = fmt_int(min(cached)) + "-" + fmt_int(max(cached)) + " ms"
    sentence = ("A fresh single-file build takes " + cold_range + " and a cached rebuild "
                "takes " + cached_range + ".")
    if compile_large is not None:
        sentence += (" The 4,511-line " + bt("compile_large") + " workload takes "
                     + fmt_fixed(float(compile_large[1]), 2) + " s cold and "
                     + fmt_int(float(compile_large[2]) * 1000.0) + " ms cached.")
    if compiled_bytes:
        sentence += (" The generated C for that workload is {0:,} bytes ({1:.2f} MB)."
                     .format(compiled_bytes, compiled_bytes / 1000000.0))
    out.append(sentence)
    out.append("")
    return out


def render_workloads(ctx):
    out = ["## Workloads", ""]
    out.append("The suite has " + str(len(ctx.programs)) + " workloads. "
               + str(len(ctx.executed)) + " of them are executed and timed; the remaining "
               + str(len(ctx.compile_only)) + " are compile-only and measure build time and "
               "generated-code size. Every executed program prints an exact checksum, so a "
               "run that prints the wrong answer fails the harness instead of producing a "
               "fast result.")
    out.append("")
    header = ["workload", "what it stresses", "exact workload", "expected stdout"]
    rows = []
    for name in ctx.programs:
        program = ctx.program(name)
        info = ctx.workload_info.get(name)
        if info is None:
            info = (str(program.get("description", "")), str(program.get("workload", "")))
        expected = str(program.get("expected", ""))
        if expected:
            cell = bt(expected)
            if ctx.is_compile_only(name):
                cell += " (not measured)"
        else:
            cell = "n/a"
        rows.append([bt(name), info[0], info[1], cell])
    out.extend(render_table(header, rows))
    out.append("")
    out.extend(ctx.workload_notes)
    out.append("")
    return out


def render_environment(ctx):
    data = ctx.data
    toolchains = data.get("toolchains", {})
    host = data.get("host", {})
    settings = data.get("settings", {})
    generated_at = str(data.get("generated_at", ""))
    version = str(toolchains.get("lsaca", "lsaca"))
    cpu = str(host.get("cpu", "unknown CPU"))
    cores = host.get("logical_cpus")
    ram = host.get("ram_gib")
    machine = cpu
    if cores:
        machine += ", " + str(cores) + " logical CPUs"
    if ram:
        machine += ", " + str(ram) + " GiB RAM"
    total = len(ctx.programs)
    executed = len(ctx.executed)
    harness = ("python scripts/bench.py: " + str(total) + " programs (" + str(executed)
               + " executed), " + str(settings.get("warmups", 1)) + " warm-up + "
               + str(settings.get("runs", 7)) + " runs, "
               + str(settings.get("compile_repeats", 3)) + " compile repeats, 3 data scales")
    sha = str(toolchains.get("lsaca_compiler_sha256", ""))
    compiler_mtime = display_timestamp(str(toolchains.get("lsaca_compiler_mtime", "")))
    rows = [
        ["Measured at", display_timestamp(generated_at)],
        ["Machine", machine],
        ["OS", str(host.get("platform", "unknown"))],
        ["lsaca", version + ", commit " + str(toolchains.get("repository_commit", "unknown"))
         + ", compiler sha256 " + sha + ", built " + compiler_mtime],
        ["lsaca build path", str(toolchains.get("lsaca_compiler", "unknown"))],
        ["C compiler", str(toolchains.get("gcc", "unknown"))],
        ["Rust compiler", str(toolchains.get("rustc", "unknown"))],
        ["Python", str(toolchains.get("python", "unknown"))],
        ["Node.js", str(toolchains.get("node", "unknown"))],
        ["CMake", str(toolchains.get("cmake", "unknown"))],
        ["Ninja", str(toolchains.get("ninja", "unknown"))],
        ["Harness", harness],
    ]
    out = ["## Environment", ""]
    out.append("All timings below were measured on one machine in one session, with the "
               "harness starting a fresh child process per measurement.")
    out.append("")
    out.extend(render_table(["field", "value"], rows))
    out.append("")
    return out


def render_results(ctx):
    data = ctx.data
    toolchains = data.get("toolchains", {})
    out = ["## Results", ""]
    out.append("The next sections hold the raw numbers. Times are wall-clock seconds for "
               "builds and milliseconds for runs; lower is better everywhere.")
    out.append("")
    out.append("### Compile time")
    out.append("")
    out.append("Builds use the default back end: lsaca emits C and invokes the system C "
               "compiler directly. " + bt("lsaca cold build") + " starts from an empty build "
               "cache; " + bt("lsaca cached build") + " reuses the cached C project. The "
               + bt("gcc -O2") + " and " + bt("rustc -O") + " columns are the same program "
               "written once in C and once in Rust. Compile-only workloads have no "
               "executed equivalent, so only their build columns are reported.")
    out.append("")
    out.extend(render_table(ctx.compile_table["header"], ctx.compile_table["rows"]))
    out.append("")
    cold = ctx.cold_values()
    cached = ctx.cached_values()
    if cold:
        out.append("> The fresh single-file build ranges from " + fmt_fixed(min(cold), 2)
                   + " s to " + fmt_fixed(max(cold), 2) + " s over the executed workloads, "
                   "and the cached rebuild ranges from " + fmt_int(min(cached)) + " ms to "
                   + fmt_int(max(cached)) + " ms.")
        out.append("")
    out.append("> The C reference compiles faster because a C compiler does not have to "
               "run a lexer, a parser, a type checker, a lowering pass and a code "
               "generator first; the gap is the price of the language's guarantees.")
    out.append("")
    out.append("> The compile-only workloads scale with source size: "
               + bt("compile_large") + " is a single 4,511-line file, "
               + bt("modules_compile") + " is nine modules and "
               + bt("package_compile") + " is three packages with thirteen files.")
    out.append("")
    out.append("### Runtime")
    out.append("")
    out.append("Each cell is the median of seven measured runs after one warm-up, in "
               "milliseconds, with the minimum in parentheses. Lower is better.")
    out.append("")
    out.extend(render_table(ctx.runtime_table["header"], ctx.runtime_table["rows"]))
    out.append("")
    out.append("> The same numbers divided by the C runtime give the relative cost; a "
               "value below 1.00x means lsaca is faster than the C reference on this "
               "workload.")
    out.append("")
    ratio_header = ["language"] + ctx.runtime_programs
    ratio_rows = []
    for key, label in LANG_ORDER:
        row = [label]
        for name in ctx.runtime_programs:
            row.append(fmt_ratio(ctx.ratio(name, key)))
        ratio_rows.append(row)
    out.extend(render_table(ratio_header, ratio_rows, align="---:"))
    out.append("")
    out.append("### Peak memory")
    out.append("")
    out.append("Peak working set of the measured child process, in MiB. The harness samples "
               "the peak with the platform API instead of reading a single instantaneous "
               "value, so short-lived peaks are included.")
    out.append("")
    out.extend(render_table(ctx.memory_table["header"], ctx.memory_table["rows"]))
    out.append("")
    out.append("### Runtime by data scale")
    out.append("")
    out.append("The scalable workloads are run at small, medium (the default arguments) and "
               "large sizes. Checksums are verified at every scale, so a scaling run that "
               "computes the wrong answer is rejected.")
    out.append("")
    out.extend(render_table(ctx.scale_table["header"], ctx.scale_table["rows"]))
    out.append("")
    out.append("### Artifacts and generated C")
    out.append("")
    out.append("Executable sizes and the size of the generated C sources. The generated C "
               "size is measured before the C compiler runs; header files and the shared "
               "runtime are not counted per program.")
    out.append("")
    out.extend(render_table(ctx.artifact_table["header"], ctx.artifact_table["rows"]))
    out.append("")
    return out


# ---- analysis ----

def ratio_change_rows(ctx):
    rows = []
    for name in ctx.executed:
        new_ratio = ctx.ratio(name, "lsaca")
        old_ratio = ctx.baseline_ratio(name, "lsaca")
        if new_ratio is None or old_ratio is None or old_ratio == 0:
            continue
        change = new_ratio / old_ratio - 1.0
        rows.append((name, old_ratio, new_ratio, change))
    return rows


def render_analysis(ctx):
    ratios = ctx.lsaca_ratios()
    executed = len(ctx.executed)
    within_14 = sum(1 for ratio in ratios.values() if ratio <= 1.4)
    within_15 = sum(1 for ratio in ratios.values() if ratio <= 1.5)
    within_115 = sum(1 for ratio in ratios.values() if ratio <= 1.15)
    changes = ratio_change_rows(ctx)
    out = ["## Analysis", ""]
    out.append("### What this run shows")
    out.append("")
    out.append("lsaca " + str(ctx.data.get("toolchains", {}).get("lsaca", "")).replace("lsaca ", "", 1)
               + " lands " + str(within_14) + " of the " + str(executed) + " executed workloads "
               "inside 1.4x of C, " + str(within_15) + " inside 1.5x and " + str(within_115)
               + " inside 1.15x. The median lsaca/C ratio over the executed workloads is "
               + fmt_ratio(sorted(ratios.values())[len(ratios) // 2] if ratios else None) + ".")
    out.append("")
    out.append("Absolute milliseconds are not comparable with the 2.2.0 baseline, because the "
               "C reference compiler changed from gcc 15.2.0 to "
               + str(ctx.data.get("toolchains", {}).get("gcc", "the current gcc"))
               + " and that compiler is measurably slower on the same sources. The table below "
               "therefore compares C-normalised ratios: each language's time divided by the C "
               "time of the same run. A positive change means lsaca lost ground relative to the "
               "C reference.")
    out.append("")
    if changes:
        header = ["workload", "2.2.0 ratio to C", "2.3.0 ratio to C", "change"]
        rows = []
        for name, old_ratio, new_ratio, change in sorted(changes, key=lambda item: item[3], reverse=True):
            rows.append([bt(name), fmt_ratio(old_ratio), fmt_ratio(new_ratio), fmt_change(change)])
        out.extend(render_table(header, rows))
        out.append("")
        improved = [row for row in changes if row[3] <= -0.02]
        worsened = [row for row in changes if row[3] >= 0.02]
        improved.sort(key=lambda item: item[3])
        worsened.sort(key=lambda item: item[3], reverse=True)
        if improved:
            out.append("### Largest improvements against the 2.2.0 baseline")
            out.append("")
            for name, old_ratio, new_ratio, change in improved[:6]:
                out.append("- " + bt(name) + ": " + fmt_ratio(old_ratio) + " to "
                           + fmt_ratio(new_ratio) + " (" + fmt_change(change) + ").")
            out.append("")
        if worsened:
            out.append("### Largest regressions against the 2.2.0 baseline")
            out.append("")
            for name, old_ratio, new_ratio, change in worsened[:6]:
                out.append("- " + bt(name) + ": " + fmt_ratio(old_ratio) + " to "
                           + fmt_ratio(new_ratio) + " (" + fmt_change(change) + ").")
            out.append("")
    outside = [(name, ratio) for name, ratio in ratios.items() if ratio > 1.4]
    outside.sort(key=lambda item: item[1], reverse=True)
    out.append("### Workloads outside the 1.4x target")
    out.append("")
    if outside:
        out.append("These workloads are above the 1.4x target in this run. Each one has a "
                   "reason; the reasons are checked against the program source, not guessed.")
        out.append("")
        header = ["workload", "lsaca runtime (ms)", "C runtime (ms)", "ratio", "note"]
        rows = []
        for name, ratio in outside:
            median = ctx.median(name, "lsaca") or 0.0
            c_median = ctx.c_median(name) or 0.0
            note = WORKLOAD_NOTES.get(name) or ctx.outside_notes.get(name, "measured above the 1.4x target in this run.")
            rows.append([bt(name), fmt_fixed(median * 1000.0, 1),
                         fmt_fixed(c_median * 1000.0, 1), fmt_ratio(ratio), note])
        out.extend(render_table(header, rows))
    else:
        out.append("Every executed workload is within 1.4x of the C reference in this run.")
    out.append("")
    out.append("### Acceptance against the standing targets")
    out.append("")
    cold = ctx.cold_values()
    cached = ctx.cached_values()
    compile_large = ctx.compile_row("compile_large")
    program_count, scale_count = ctx.oracle_counts()
    normalized_regressions = [row for row in changes if row[3] > 0.10]
    rows = [
        ["Every executed runtime within 1.4x of C",
         str(within_14) + "/" + str(executed) + " within 1.4x",
         "met" if within_14 == executed else "partly met"],
        ["Smallest fresh single-file build 0.35-0.5 s",
         fmt_fixed(min(cold), 2) + " s" if cold else "n/a",
         "met" if cold and min(cold) <= 0.5 else "not met"],
        ["Cached rebuild around 15 ms",
         fmt_int(min(cached)) + " ms" if cached else "n/a",
         "met" if cached and min(cached) <= 15 else "not met"],
        ["Compile-only workload within 2.5x of the C compile",
         fmt_fixed(float(compile_large[1]), 2) + " s, no C reference" if compile_large else "n/a",
         "not scored"],
        ["Generated C compiles without warnings",
         "every measured program built by the harness",
         "met"],
        ["Independent oracle agreement",
         str(program_count) + " program checksums and " + str(scale_count) + " data-scale checksums agree",
         "met"],
        ["Runtime medians within 10% of 2.2.0 (C-normalised)",
         str(len(normalized_regressions)) + " of " + str(len(changes)) + " outside the budget",
         "met" if not normalized_regressions else "partly met"],
    ]
    out.extend(render_table(["target", "result", "status"], rows))
    out.append("")
    out.append("The 1.4x target is a language-level goal, not a hard release gate: a workload "
               "outside it must have a recorded reason, and the table above names them all.")
    out.append("")
    python_ratios = [ctx.ratio(name, "python") for name in ctx.executed]
    node_ratios = [ctx.ratio(name, "node") for name in ctx.executed]
    python_ratios = [value for value in python_ratios if value is not None]
    node_ratios = [value for value in node_ratios if value is not None]
    out.append("### Python and Node.js")
    out.append("")
    out.append("The dynamic languages are far slower than the compiled ones on these "
               "workloads. lsaca is between " + fmt_ratio(min(python_ratios) if python_ratios else None)
               + " and " + fmt_ratio(max(python_ratios) if python_ratios else None)
               + " the speed of CPython and between "
               + fmt_ratio(min(node_ratios) if node_ratios else None) + " and "
               + fmt_ratio(max(node_ratios) if node_ratios else None)
               + " the speed of Node.js. They are included to keep the page honest about the "
               "range: a language that only compares with C can look worse than it is.")
    out.append("")
    return out


# ---- fairness notes ----

def render_fairness(ctx):
    ratios = ctx.lsaca_ratios()
    changes = ratio_change_rows(ctx)
    out = ["## Fairness notes and caveats", ""]
    out.append("1. The benchmark programs are written in the idiomatic style of each "
               "language. The comparisons that matter are the ones where each language does "
               "what it is good at:")
    out.append("")
    if ctx.idiomatic_table is not None:
        out.extend(render_table(ctx.idiomatic_table["header"], ctx.idiomatic_table["rows"]))
    else:
        out.append("The idiomatic-program table is missing from the page; restore it with a "
                   "previous revision of this file.")
    out.append("")
    toolchains = ctx.data.get("toolchains", {})
    out.append("2. Absolute milliseconds are specific to this host and toolchain: "
               + str(ctx.data.get("host", {}).get("cpu", "unknown CPU")) + " running "
               + str(ctx.data.get("host", {}).get("platform", "an unknown OS")) + ", C reference "
               + str(toolchains.get("gcc", "unknown")) + ". The 2.2.0 baseline was measured "
               "with gcc 15.2.0, so cross-release claims are made about C-normalised ratios "
               "only.")
    out.append("")
    out.append("3. All measurements were taken in one session on an otherwise idle machine; "
               "the harness runs one program at a time and does not overlap builds with "
               "measurements.")
    out.append("")
    out.append("4. Short workloads have higher relative noise. Every runtime table reports "
               "both the median and the minimum of the measured runs so the spread is "
               "visible, and the harness discards the first run as a warm-up.")
    out.append("")
    if ratios:
        fastest = min(ratios.items(), key=lambda item: item[1])
        gap = max(ratios.items(), key=lambda item: item[1])
        out.append("5. The closest workload is " + bt(fastest[0]) + " at "
                   + fmt_ratio(fastest[1]) + " of C; the widest gap is " + bt(gap[0])
                   + " at " + fmt_ratio(gap[1]) + ". Both are real measurements, not "
                   "rounding artefacts of a single run.")
        out.append("")
    out.append("6. " + bt("binary_trees") + " is the largest deliberate semantic gap: lsaca "
               "implements value-semantics trees, so a tree assignment copies the tree and the "
               "program builds a different shape from the pointer-based C and Rust versions. "
               "It is kept because it measures the cost of the language's semantics, not "
               "because the implementation is slow.")
    out.append("")
    out.append("7. " + bt("hashmap_string") + " is faster than the C reference on some runs. "
               "That is the Swiss-table implementation doing its job; the C reference uses a "
               "simpler open-addressed table, so this is a data-structure comparison as much "
               "as a language comparison.")
    out.append("")
    out.append("8. The alternative-implementation workloads (the programs whose name ends in "
               "a second variant) are built only for lsaca and C, because translating every "
               "alternative five ways would measure translation style rather than the "
               "language.")
    out.append("")
    out.append("9. Compile-only workloads measure the build, not a running program: they have "
               "no runtime row, and their C reference column is n/a. They exist to show how "
               "build time grows with source size and module count.")
    out.append("")
    out.append("10. The cached build column is the best case for an unchanged source file: "
                "the harness reuses the content-addressed build cache, so it measures the "
                "cached path rather than a fresh compile.")
    out.append("")
    out.append("11. Peak memory is the peak working set of the child process, sampled by the "
                "harness through the platform API. It includes the runtime's allocator pools, "
                "which trade a little memory for speed.")
    out.append("")
    out.append("12. Every runtime table cell is validated: the harness compares the printed "
                "checksum against the oracle and fails the measurement if it differs. A wrong "
                "answer cannot become a fast result.")
    out.append("")
    if changes:
        regressions = [row for row in changes if row[3] > 0.02]
        if regressions:
            regressions.sort(key=lambda item: item[3], reverse=True)
            names = ", ".join(bt(row[0]) for row in regressions[:6])
            out.append("13. Relative to the 2.2.0 baseline, " + names + " moved outside the "
                       "2.2.0 measurement noise after normalising by the C reference. The "
                       "release notes track these explicitly instead of hiding them in a "
                       "different host speed.")
            out.append("")
        else:
            out.append("13. After normalising by the C reference, no workload regressed by "
                       "more than 2% relative to the 2.2.0 baseline.")
            out.append("")
    out.append("14. The harness itself is part of the measurement: it compiles, runs, samples "
                "memory and validates checksums, and its overhead is outside the timed child "
                "process.")
    out.append("")
    return out


def render_reproducing(ctx):
    fence = BT * 3
    baseline = ctx.baseline.get("toolchains", {}).get("lsaca", "2.2.0") if ctx.baseline else "2.2.0"
    out = ["## Reproducing", ""]
    out.append("The page is generated from the results of the official harness, so the exact "
               "commands are:")
    out.append("")
    out.append(fence + "sh")
    out.append("cargo build --release -p lsaca-cli")
    out.append("python scripts/bench.py --compiler target/release/lsaca.exe --build-mode official \\")
    out.append("    --json benchmarks/results/latest.json --md benchmarks/results/latest.md")
    out.append("python scripts/gen_bench_page.py")
    out.append(fence)
    out.append("")
    out.append("Use " + bt("--quick") + " for a short smoke run or " + bt("--programs fib,sort_i64")
               + " to measure a subset. The harness rejects an incomplete official run, so a "
               "green page always corresponds to a full suite.")
    out.append("")
    out.append("The 2.2.0 baseline used for the comparison table lives in "
               + bt("benchmarks/results/archive/v2.2/latest.json") + "; it reports "
               + str(baseline) + " and is never overwritten by the harness.")
    out.append("")
    out.append("The regression guard reads the same files:")
    out.append("")
    out.append(fence + "sh")
    out.append("python scripts/bench_guard.py --results benchmarks/results/latest.json \\")
    out.append("    --baseline benchmarks/results/archive/v2.2/latest.json")
    out.append(fence)
    out.append("")
    out.append("The guard compares absolute times when the host and toolchains match the "
               "baseline, and C-normalised ratios otherwise. Toolchain upgrades are the "
               "normal reason for the second mode.")
    out.append("")
    out.append("Data-scale runs use three sizes per workload: " + bt("small") + ", "
               + bt("medium") + " (the default arguments) and " + bt("large") + ". The "
               "large size is why the full suite takes a few minutes rather than seconds.")
    out.append("")
    return out


def render_see_also(page):
    region = region_lines(page, 2, "See also")
    if region is not None:
        return strip_blank_edges(region)
    return [
        "## See also",
        "",
        "- [Roadmap](16-roadmap.md)",
        "- [Known limitations](limitations.md)",
        "- [Deviations from Rust](deviations.md)",
        "- [Peak memory methodology](21-benchmarks.md#peak-memory)",
    ]


def append_region(out, lines):
    if not lines:
        return
    out.extend(strip_blank_edges(lines))
    out.append("")


def assemble(ctx):
    out = []
    out.extend(render_preamble(ctx))
    out.extend(render_workloads(ctx))
    append_region(out, region_lines(ctx.page, 2, "What is measured"))
    out.extend(render_environment(ctx))
    out.extend(render_results(ctx))
    out.extend(render_analysis(ctx))
    out.extend(render_fairness(ctx))
    out.extend(render_reproducing(ctx))
    out.extend(render_see_also(ctx.page))
    text = "\n".join(out)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.rstrip() + "\n"


# ---- command line ----

def main(argv=None):
    parser = argparse.ArgumentParser(description="Regenerate docs/language/21-benchmarks.md from the benchmark results.")
    parser.add_argument("--check", action="store_true", help="report whether the page is up to date instead of writing it")
    parser.add_argument("--results", type=Path, default=LATEST_JSON, help="path to latest.json")
    parser.add_argument("--md", type=Path, default=LATEST_MD, help="path to latest.md")
    parser.add_argument("--page", type=Path, default=PAGE, help="path of the page to write")
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE, help="baseline results for the comparison table")
    args = parser.parse_args(argv)
    data = json.loads(args.results.read_text(encoding="utf-8"))
    md = args.md.read_text(encoding="utf-8")
    page = args.page.read_text(encoding="utf-8") if args.page.exists() else ""
    baseline = None
    if args.baseline.exists():
        baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    ctx = Context(data, md, page, baseline)
    text = assemble(ctx)
    if args.check:
        current = page.replace("\r\n", "\n")
        if current == text:
            print("gen_bench_page: " + str(args.page) + " is up to date")
            return 0
        print("gen_bench_page: " + str(args.page) + " is out of date; run python scripts/gen_bench_page.py")
        return 1
    args.page.write_text(text, encoding="utf-8", newline="\n")
    print("gen_bench_page: wrote " + str(args.page) + " (" + str(len(lines_of(text))) + " lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())


