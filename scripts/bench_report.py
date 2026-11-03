#!/usr/bin/env python3
"""Render the v2.0 benchmark report sections from a scripts/bench.py result JSON.

The output is markdown meant to be pasted into docs/language/21-benchmarks.md
after the existing compile / runtime / peak-memory / ratio tables, which stay
byte-identical to benchmarks/results/latest.md so scripts/check_bench21.py keeps
validating them. This script adds the v2.0 material:

  * run identity (compiler SHA-256, commit, host, settings);
  * a workload x language table with runtime, peak memory, executable bytes and
    cold-build columns;
  * build artefacts and compile times for every program, compile-only included;
  * the small/medium/large runtime-by-data-scale table;
  * the independent-oracle verification summary;
  * a before/after comparison against an archived earlier result (default:
    benchmarks/results/archive/v1.7/latest.json), plus the v2.0 acceptance
    target checks.

Usage (from the repository root, after an official run):

    python scripts/bench_report.py
    python scripts/bench_report.py --json benchmarks/results/latest.json \
        --baseline benchmarks/results/archive/v1.7/latest.json \
        --out benchmarks/results/v2-report-sections.md

The generated file is not published by itself: docs-verifier / the Lead paste the
sections into the manual page after the official run. Nothing here writes
latest.json or latest.md.
"""

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_JSON = REPO_ROOT / "benchmarks" / "results" / "latest.json"
DEFAULT_BASELINE = REPO_ROOT / "benchmarks" / "results" / "archive" / "v1.7" / "latest.json"
DEFAULT_OUT = REPO_ROOT / "benchmarks" / "results" / "v2-report-sections.md"

LANGUAGES = ["lsaca", "c", "rust", "python", "node"]
LANGUAGE_LABELS = {
    "lsaca": "lsaca",
    "c": "C (gcc -O2)",
    "rust": "Rust (rustc -O)",
    "python": "Python 3",
    "node": "Node.js",
}
COMPILED = ["lsaca", "c", "rust"]
RATIO_TARGET = 1.4


def load_json(path):
    path = Path(path)
    if not path.exists():
        return None
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def fmt_seconds(value):
    if value is None:
        return "n/a"
    if value >= 10.0:
        return "%.2f" % value
    return "%.3f" % value


def fmt_ms(value):
    if value is None:
        return "n/a"
    return "%.1f" % (value * 1000.0)


def fmt_memory(record):
    if not record or record.get("peak_mib") is None:
        return "n/a"
    return "%.1f" % record["peak_mib"]


def fmt_bytes(value):
    if value is None:
        return "n/a"
    return "{:,}".format(int(value))


def fmt_change(old, new):
    if not old or new is None:
        return "n/a"
    change = (new / old - 1.0) * 100.0
    sign = "+" if change >= 0 else ""
    return "%s%.1f%%" % (sign, change)


def language_records(entry):
    return entry.get("languages") or {}


def compile_record(entry, language):
    return (language_records(entry).get(language) or {}).get("compile") or {}


def runtime_record(entry, language):
    return (language_records(entry).get(language) or {}).get("runtime") or {}


def memory_record(entry, language):
    return (language_records(entry).get(language) or {}).get("memory") or {}


def cold_seconds(record):
    """Cold build median for lsaca (official/fallback) or single native build."""
    if not record:
        return None
    if record.get("cold_median_s") is not None:
        return record["cold_median_s"]
    if record.get("cold_s") is not None:
        return record["cold_s"]
    if record.get("kind") == "lsaca-fallback" and record.get("total_s") is not None:
        return record["total_s"]
    if record.get("median_s") is not None:
        return record["median_s"]
    return None


def cached_seconds(record):
    if not record:
        return None
    if record.get("cached_median_s") is not None:
        return record["cached_median_s"]
    return None


def exe_bytes(entry, language):
    record = compile_record(entry, language)
    return record.get("exe_bytes")


def program_order(results):
    programs = results.get("programs") or {}
    order = (results.get("settings") or {}).get("programs")
    if order:
        return [name for name in order if name in programs]
    return list(programs)


def programs_of(results):
    return results.get("programs") or {}


def run_languages(results):
    configured = (results.get("settings") or {}).get("languages") or []
    return [language for language in LANGUAGES if language in configured]


def is_compile_only(entry):
    return bool(entry.get("compile_only"))


def entry_project(entry):
    return entry.get("project")


def ratio(entry, language="c"):
    lsaca = runtime_record(entry, "lsaca").get("median_s")
    other = runtime_record(entry, language).get("median_s")
    if not lsaca or not other:
        return None
    return lsaca / other


def render_identity(results, source_path, baseline_path, label):
    host = results.get("host") or {}
    tools = results.get("toolchains") or {}
    settings = results.get("settings") or {}
    lines = [
        "<!-- Generated by scripts/bench_report.py; source: %s; baseline: %s."
        % (source_path, baseline_path or "none"),
        "     Do not edit by hand; re-run the script after every official suite run. -->",
        "",
        "## v2.0 benchmark run identity",
        "",
    ]
    if label:
        lines.append("> **%s**" % label)
        lines.append("")
    lines.extend(
        [
            "- Generated: %s" % results.get("generated_at"),
            "- Compiler: %s (SHA-256 prefix %s, mtime %s)"
            % (
                tools.get("lsaca"),
                tools.get("lsaca_compiler_sha256"),
                tools.get("lsaca_compiler_mtime"),
            ),
            "- Repository commit: %s" % tools.get("repository_commit"),
            "- Host: %s; CPU: %s (%s logical CPUs); RAM: %s GiB"
            % (
                host.get("platform"),
                host.get("cpu") or "unknown",
                host.get("logical_cpus"),
                host.get("ram_gib"),
            ),
            "- Build mode: %s; compile repeats: %s; runtime runs: %s; "
            "runtime warmups: %s; scale runs: %s"
            % (
                (results.get("lsaca_build") or {}).get("mode", "n/a"),
                settings.get("compile_repeats"),
                settings.get("runs"),
                settings.get("warmups"),
                settings.get("scale_runs", "n/a"),
            ),
            "- Languages: %s" % ", ".join(run_languages(results)),
            "",
            "Every number below comes from this one run on the host above, so the"
            " file is a single-machine measurement and must not be compared with"
            " numbers from another machine.",
            "",
        ]
    )
    return lines


def render_workload_detail(results):
    lines = [
        "## Workload detail: runtime, peak memory, executable size, cold build",
        "",
        "| workload | language | runtime median (ms) | peak memory (MiB) | "
        "executable bytes | cold build (s) |",
        "| --- | --- | ---: | ---: | ---: | ---: |",
    ]
    for name in program_order(results):
        entry = programs_of(results)[name]
        if is_compile_only(entry):
            continue
        for language in run_languages(results):
            runtime = runtime_record(entry, language)
            memory = memory_record(entry, language)
            compile = compile_record(entry, language)
            if language in COMPILED:
                built = exe_bytes(entry, language)
                exe_cell = fmt_bytes(built)
                cold_cell = fmt_seconds(cold_seconds(compile))
            else:
                exe_cell = "n/a (interpreted)"
                cold_cell = "n/a (interpreted)"
            lines.append(
                "| "
                + " | ".join(
                    [
                        name,
                        LANGUAGE_LABELS.get(language, language),
                        fmt_ms(runtime.get("median_s")),
                        fmt_memory(memory),
                        exe_cell,
                        cold_cell,
                    ]
                )
                + " |"
            )
    lines.extend(
        [
            "",
            "> Runtime: median of --runs measured runs (default 7); the 95% confidence"
            " interval, standard deviation and any outlier samples are in latest.json.",
            "> Peak memory: highest sampled working set while the program ran, per"
            " language; interpreted languages therefore include their VM.",
            "> Executable: the built binary file in bytes (lsaca, C, Rust only)."
            " Python and Node.js are interpreted and have no ahead-of-time build.",
            "> Cold build: median of --compile-repeats cold builds (default 3) from a"
            " fresh cache directory for lsaca; one --compile-repeats median for gcc"
            " and rustc. In fallback build mode the lsaca cold column is the"
            " emit-c + gcc total.",
            "",
        ]
    )
    return lines


def render_build_artifacts(results):
    lines = [
        "## Build artefacts and compile time (lsaca)",
        "",
        "| workload | kind | generated C bytes | generated C files | "
        "lsaca executable bytes | lsaca cold build (s) | lsaca cached build (s) | "
        "C executable bytes | Rust executable bytes |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for name in program_order(results):
        entry = programs_of(results)[name]
        lsaca_compile = compile_record(entry, "lsaca")
        c_compile = compile_record(entry, "c")
        rust_compile = compile_record(entry, "rust")
        lines.append(
            "| "
            + " | ".join(
                [
                    name,
                    "compile-only" if is_compile_only(entry) else "executed",
                    fmt_bytes(lsaca_compile.get("generated_c_bytes")),
                    fmt_bytes(lsaca_compile.get("generated_c_files")),
                    fmt_bytes(lsaca_compile.get("exe_bytes")),
                    fmt_seconds(cold_seconds(lsaca_compile)),
                    fmt_seconds(cached_seconds(lsaca_compile)),
                    fmt_bytes(c_compile.get("exe_bytes")),
                    fmt_bytes(rust_compile.get("exe_bytes")),
                ]
            )
            + " |"
        )
    lines.extend(
        [
            "",
            "> Generated C: bytes of the emitted C sources; for multi-file and package"
            " projects the column is the sum over every generated translation unit and"
            " the file count is shown next to it.",
            "> Compile-only workloads are built but not run, so they have no runtime,"
            " peak-memory or executable-output row. Cached builds reuse the project"
            " cache and mostly re-link the executable.",
            "",
        ]
    )
    return lines


def render_scales(results):
    settings = results.get("settings") or {}
    scale_names = settings.get("scales") or []
    if not scale_names:
        return []
    languages = [
        language
        for language in run_languages(results)
        if any(
            language in ((entry.get("scales") or {}).get(scale_name, {}).get("languages") or {})
            for entry in programs_of(results).values()
            for scale_name in scale_names
        )
    ]
    top = (
        "| workload | scale | args | expected | "
        + " | ".join("%s (ms)" % LANGUAGE_LABELS.get(language, language) for language in languages)
        + " |"
    )
    align = "| --- | --- | --- | --- | " + " | ".join("---:" for _ in languages) + " |"
    lines = [
        "## Runtime by data scale (median ms)",
        "",
        top,
        align,
    ]
    for name in program_order(results):
        entry = programs_of(results)[name]
        scales = entry.get("scales") or {}
        for scale_name in scale_names:
            scale = scales.get(scale_name)
            if not scale:
                continue
            args = scale.get("args") or []
            cells = [
                name,
                scale_name,
                " ".join(args) if args else "(driver default)",
                scale.get("expected") or "n/a",
            ]
            for language in languages:
                record = ((scale.get("languages") or {}).get(language) or {}).get("runtime") or {}
                cells.append(fmt_ms(record.get("median_s")))
            lines.append("| " + " | ".join(cells) + " |")
    lines.extend(
        [
            "",
            "> The medium scale is the default workload configuration and is the same"
            " measurement as the primary runtime table above; small and large use"
            " program arguments. Every expected line is recomputed by"
            " scripts/oracles.py, and every language is validated byte-for-byte"
            " before it is timed. Scale runs use no warmup and --scale-runs runs"
            " (default 3).",
            "",
        ]
    )
    return lines


def render_oracles(results):
    report = results.get("oracle_verification")
    lines = ["## Independent oracle verification", ""]
    if not report:
        lines.extend(
            [
                "No oracle report in this JSON (this result was produced by an older"
                " harness). Re-run the suite with scripts/oracles.py available so the"
                " checksums are verified independently.",
                "",
            ]
        )
        return lines
    program_lines = report.get("programs") or []
    scales = report.get("scales") or {}
    scale_count = sum(len(entries or {}) for entries in scales.values())
    mismatches = report.get("mismatches") or []
    lines.append(
        "scripts/oracles.py holds independent Python reference implementations of"
        " every workload. Before measuring, the harness recomputed %d program"
        " checksums and %d data-scale checksums from those references; mismatches:"
        " %d." % (len(program_lines), scale_count, len(mismatches))
    )
    lines.append("")
    if mismatches:
        lines.append("Mismatches:")
        for mismatch in mismatches:
            lines.append("- %s" % mismatch)
        lines.append("")
    else:
        lines.append(
            "A mismatch aborts the run before any measurement is taken, so a"
            " published result always has all language outputs agreeing with the"
            " independent reference."
        )
        lines.append("")
    return lines


def baseline_identity(baseline):
    tools = baseline.get("toolchains") or {}
    return "baseline: %s, compiler %s, SHA-256 prefix %s, commit %s" % (
        baseline.get("generated_at"),
        tools.get("lsaca"),
        tools.get("lsaca_compiler_sha256"),
        tools.get("repository_commit"),
    )


def render_before_after(results, baseline):
    lines = ["## Before and after: v1.7 to v2.0", ""]
    if not baseline:
        lines.extend(
            [
                "_Baseline JSON not found; pass --baseline to compare against an"
                " archived result._",
                "",
            ]
        )
        return lines
    lines.append("> " + baseline_identity(baseline))
    lines.append("")
    current_programs = programs_of(results)
    baseline_programs = programs_of(baseline)
    common = [
        name
        for name in program_order(results)
        if name in baseline_programs
        and not is_compile_only(current_programs[name])
        and not is_compile_only(baseline_programs[name])
    ]
    lines.extend(
        [
            "### Runtime medians (ms, lsaca)",
            "",
            "| workload | v1.7 lsaca | v2.0 lsaca | change | v1.7 x C | v2.0 x C |",
            "| --- | ---: | ---: | ---: | ---: | ---: |",
        ]
    )
    above_target = []
    for name in common:
        old_entry = baseline_programs[name]
        new_entry = current_programs[name]
        old_ms = runtime_record(old_entry, "lsaca").get("median_s")
        new_ms = runtime_record(new_entry, "lsaca").get("median_s")
        old_ratio = ratio(old_entry)
        new_ratio = ratio(new_entry)
        if new_ratio is not None and new_ratio > RATIO_TARGET:
            above_target.append((name, new_ratio))
        lines.append(
            "| "
            + " | ".join(
                [
                    name,
                    fmt_ms(old_ms),
                    fmt_ms(new_ms),
                    fmt_change(old_ms, new_ms),
                    "%.2f" % old_ratio if old_ratio is not None else "n/a",
                    "%.2f" % new_ratio if new_ratio is not None else "n/a",
                ]
            )
            + " |"
        )
    lines.append("")
    if above_target:
        offenders = ", ".join("%s (%.2fx)" % (name, value) for name, value in above_target)
        lines.append(
            "v2.0 is within %.1fx of C on %d of %d compared workloads; above the"
            " target: %s." % (RATIO_TARGET, len(common) - len(above_target), len(common), offenders)
        )
    else:
        lines.append(
            "v2.0 is within %.1fx of C on all %d compared workloads."
            % (RATIO_TARGET, len(common))
        )
    lines.append("")

    lines.extend(
        [
            "### Compile medians (lsaca build -r)",
            "",
            "| workload | v1.7 cold (s) | v2.0 cold (s) | change | "
            "v1.7 cached (ms) | v2.0 cached (ms) | change |",
            "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
        ]
    )
    compile_common = [
        name
        for name in program_order(results)
        if name in baseline_programs
    ]
    for name in compile_common:
        old_compile = compile_record(baseline_programs[name], "lsaca")
        new_compile = compile_record(current_programs[name], "lsaca")
        old_cold = cold_seconds(old_compile)
        new_cold = cold_seconds(new_compile)
        old_cached = cached_seconds(old_compile)
        new_cached = cached_seconds(new_compile)
        lines.append(
            "| "
            + " | ".join(
                [
                    name,
                    fmt_seconds(old_cold),
                    fmt_seconds(new_cold),
                    fmt_change(old_cold, new_cold),
                    fmt_ms(old_cached),
                    fmt_ms(new_cached),
                    fmt_change(old_cached, new_cached),
                ]
            )
            + " |"
        )
    lines.append("")

    lines.extend(render_target_checks(results, baseline))
    return lines


def render_target_checks(results, baseline):
    lines = [
        "### v2.0 acceptance targets",
        "",
        "| metric | v1.7 | v2.0 | target | status |",
        "| --- | ---: | ---: | --- | --- |",
    ]
    current_programs = programs_of(results)
    baseline_programs = programs_of(baseline)
    current_single = [
        name
        for name in program_order(results)
        if not is_compile_only(current_programs[name]) and not entry_project(current_programs[name])
    ]
    baseline_single = [
        name
        for name in current_single
        if name in baseline_programs and not is_compile_only(baseline_programs[name])
    ]

    def min_cold(programs, entries):
        values = []
        for name in programs:
            value = cold_seconds(compile_record(entries[name], "lsaca"))
            if value is not None:
                values.append((name, value))
        if not values:
            return None, None
        return min(values, key=lambda item: item[1])

    old_name, old_cold = min_cold(baseline_single, baseline_programs)
    new_name, new_cold = min_cold(current_single, current_programs)

    def status_between(value, low, high):
        if value is None:
            return "n/a"
        if low <= value <= high:
            return "within target"
        return "outside target"

    lines.append(
        "| smallest single-file cold build (%s) | %s (%s) | %s (%s) | "
        "0.35-0.5 s | %s |"
        % (
            new_name or "n/a",
            fmt_seconds(old_cold),
            old_name or "n/a",
            fmt_seconds(new_cold),
            new_name or "n/a",
            status_between(new_cold, 0.35, 0.5),
        )
    )
    fib_old = cold_seconds(compile_record(baseline_programs.get("fib") or {}, "lsaca"))
    fib_cached_old = cached_seconds(compile_record(baseline_programs.get("fib") or {}, "lsaca"))
    fib_cached_new = cached_seconds(compile_record(current_programs.get("fib") or {}, "lsaca"))
    cache_status = "n/a"
    if fib_cached_new is not None:
        cache_status = "within target" if fib_cached_new <= 0.020 else "outside target"
    lines.append(
        "| cached build, fib | %s ms | %s ms | about 15 ms | %s |"
        % (fmt_ms(fib_cached_old), fmt_ms(fib_cached_new), cache_status)
    )
    compile_large_old = cold_seconds(compile_record(baseline_programs.get("compile_large") or {}, "lsaca"))
    compile_large_new = cold_seconds(compile_record(current_programs.get("compile_large") or {}, "lsaca"))
    lines.append(
        "| compile_large cold build | %s s | %s s | <= 2.5 x the C reference build | "
        "needs C reference (measure separately) |"
        % (fmt_seconds(compile_large_old), fmt_seconds(compile_large_new))
    )
    above_target = [
        name
        for name in program_order(results)
        if not is_compile_only(current_programs[name]) and (ratio(current_programs[name]) or 0) > RATIO_TARGET
    ]
    lines.append(
        "| runtime within %.1fx of C | %s | %d of %d above target | none above target "
        "unless documented | %s |"
        % (
            RATIO_TARGET,
            "n/a",
            len(above_target),
            sum(1 for name in program_order(results) if not is_compile_only(current_programs[name]) and ratio(current_programs[name]) is not None),
            "pass" if not above_target else "document: " + ", ".join(above_target),
        )
    )
    lines.append("")
    lines.append(
        "Targets are the v2.0 performance acceptance criteria: every measured aspect"
        " within 1.4x of C (or documented), a hello-sized cold build of 0.35-0.5 s,"
        " a cached build of about 15 ms, and compile_large at no more than 2.5x the"
        " reference C build. The cold-build proxy is the smallest single-file"
        " workload in the suite; once a dedicated hello workload is added, use it"
        " instead."
    )
    lines.append("")
    return lines


KNOWN_LIMITATIONS = {
    "binary_trees": (
        "by-value workload: each recursive step builds and returns a subtree by value, "
        "so deep copies are part of the measured workload by design; "
        "`binary_trees_refs` is the comparable reference-traversal variant."
    ),
}


def _scale_runtime(entry, scale):
    scales = entry.get("scales") or {}
    record = (scales.get(scale) or {}).get("languages") or {}
    return (record.get("lsaca") or {}).get("runtime") or {}


def render_three_way(results, baseline, preperf):
    """v1.7 archive | Phase D pre-perf baseline | v2.0 final, plus exceptions."""
    if not preperf:
        return []
    lines = [
        "### Three-way before/after: v1.7, Phase D baseline, v2.0 final",
        "",
        "> Phase D baseline: %s. That host was loaded by parallel builds, so treat those"
        " medians as directional." % baseline_identity(preperf),
        "",
    ]
    current = programs_of(results)
    old = programs_of(baseline) if baseline else {}
    pre = programs_of(preperf)
    names = [
        name
        for name in program_order(results)
        if name in current and not is_compile_only(current[name])
    ]

    lines.extend(
        [
            "### Runtime medians: three-way comparison (ms, lsaca)",
            "",
            "| workload | v1.7 | Phase D baseline | v2.0 final | final vs v1.7 | final x C |",
            "| --- | ---: | ---: | ---: | ---: | ---: |",
        ]
    )
    exceptions = []
    for name in names:
        old_ms = None
        if name in old and not is_compile_only(old[name]):
            old_ms = runtime_record(old[name], "lsaca").get("median_s")
        pre_ms = runtime_record(pre[name], "lsaca").get("median_s") if name in pre else None
        new_ms = runtime_record(current[name], "lsaca").get("median_s")
        new_ratio = ratio(current[name])
        if new_ratio is not None and new_ratio > RATIO_TARGET:
            exceptions.append((name, new_ratio))
        lines.append(
            "| "
            + " | ".join(
                [
                    name,
                    fmt_ms(old_ms),
                    fmt_ms(pre_ms),
                    fmt_ms(new_ms),
                    fmt_change(old_ms, new_ms),
                    "%.2f" % new_ratio if new_ratio is not None else "n/a",
                ]
            )
            + " |"
        )
    lines.append("")

    compile_names = [
        name
        for name in names
        if name in pre
        and compile_record(pre[name], "lsaca")
        and compile_record(current[name], "lsaca")
    ]
    if compile_names:
        lines.extend(
            [
                "### Compile medians: three-way comparison (lsaca build -r)",
                "",
                "| workload | v1.7 cold (s) | Phase D baseline cold (s) | v2.0 final cold (s) "
                "| final vs v1.7 | Phase D baseline cached (ms) | v2.0 final cached (ms) |",
                "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
            ]
        )
        for name in compile_names:
            old_compile = compile_record(old[name], "lsaca") if name in old else {}
            pre_compile = compile_record(pre[name], "lsaca")
            new_compile = compile_record(current[name], "lsaca")
            old_cold = cold_seconds(old_compile)
            new_cold = cold_seconds(new_compile)
            lines.append(
                "| "
                + " | ".join(
                    [
                        name,
                        fmt_seconds(old_cold),
                        fmt_seconds(cold_seconds(pre_compile)),
                        fmt_seconds(new_cold),
                        fmt_change(old_cold, new_cold),
                        fmt_ms(cached_seconds(pre_compile)),
                        fmt_ms(cached_seconds(new_compile)),
                    ]
                )
                + " |"
            )
        lines.append("")

    final_scales = results.get("settings", {}).get("scales") or []
    pre_scales = preperf.get("settings", {}).get("scales") or []
    scales = [scale for scale in final_scales if scale in pre_scales]
    if scales:
        lines.extend(
            [
                "### Runtime by data scale: Phase D baseline vs v2.0 final (median ms)",
                "",
                "| workload | scale | args | Phase D baseline | v2.0 final | change |",
                "| --- | --- | --- | ---: | ---: | ---: |",
            ]
        )
        for name in names:
            if name not in pre:
                continue
            for scale in scales:
                final_scale = (current[name].get("scales") or {}).get(scale) or {}
                pre_scale = (pre[name].get("scales") or {}).get(scale) or {}
                final_rt = _scale_runtime(current[name], scale)
                pre_rt = _scale_runtime(pre[name], scale)
                final_ms = final_rt.get("median_s")
                pre_ms = pre_rt.get("median_s")
                if final_ms is None and pre_ms is None:
                    continue
                args = final_scale.get("args") or pre_scale.get("args") or []
                lines.append(
                    "| "
                    + " | ".join(
                        [
                            name,
                            scale,
                            " ".join(str(item) for item in args) or "(default)",
                            fmt_ms(pre_ms),
                            fmt_ms(final_ms),
                            fmt_change(pre_ms, final_ms),
                        ]
                    )
                    + " |"
                )
        lines.append("")

    lines.extend(["### Documented exceptions and limitations", ""])
    if exceptions:
        lines.extend(
            [
                "v2.0 final is above the %.1fx-of-C target on %d of %d compared workloads."
                % (RATIO_TARGET, len(exceptions), len(names)),
                "",
                "| workload | x C | documented reason |",
                "| --- | ---: | --- |",
            ]
        )
        for name, value in exceptions:
            lines.append(
                "| %s | %.2f | %s |"
                % (name, value, KNOWN_LIMITATIONS.get(name, "to be documented in Fairness notes"))
            )
        lines.append("")
    else:
        lines.append(
            "v2.0 final is within %.1fx of C on all %d compared workloads."
            % (RATIO_TARGET, len(names))
        )
        lines.append("")
    improved = []
    for name in names:
        if name not in pre:
            continue
        pre_ratio = ratio(pre[name])
        final_ratio = ratio(current[name])
        if (
            pre_ratio is not None
            and pre_ratio > RATIO_TARGET
            and final_ratio is not None
            and final_ratio <= RATIO_TARGET
        ):
            improved.append((name, pre_ratio, final_ratio))
    if improved:
        lines.append(
            "Brought under the target by Phase D: "
            + ", ".join(
                "%s (%.2fx -> %.2fx)" % (name, before, after)
                for name, before, after in improved
            )
            + "."
        )
        lines.append("")
    documented = [
        (name, KNOWN_LIMITATIONS[name])
        for name in names
        if name in KNOWN_LIMITATIONS
    ]
    if documented:
        lines.append("Known semantic limitations (documented, not chased):")
        lines.append("")
        for name, note in documented:
            lines.append("- `%s`: %s" % (name, note))
        lines.append("")
    return lines


def render(results, baseline, baseline2, source_path, baseline_path, label):
    lines = []
    lines.extend(render_identity(results, source_path, baseline_path, label))
    lines.extend(render_workload_detail(results))
    lines.extend(render_build_artifacts(results))
    lines.extend(render_scales(results))
    lines.extend(render_oracles(results))
    lines.extend(render_before_after(results, baseline))
    lines.extend(render_three_way(results, baseline, baseline2))
    return "\n".join(lines).rstrip() + "\n"


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Render the v2.0 benchmark report sections from a bench.py JSON result."
    )
    parser.add_argument("--json", default=str(DEFAULT_JSON), help="current result JSON (default: %(default)s)")
    parser.add_argument(
        "--baseline",
        default=str(DEFAULT_BASELINE),
        help="archived earlier result JSON for the before/after section (default: %(default)s)",
    )
    parser.add_argument(
        "--baseline2",
        default="",
        help="optional second archived result JSON (e.g. the Phase D pre-perf baseline)"
        " for a three-way before/after section; empty disables it",
    )
    parser.add_argument("--out", default=str(DEFAULT_OUT), help="markdown output path (default: %(default)s)")
    parser.add_argument("--label", default="", help="extra label printed under the title (for dry runs)")
    parser.add_argument("--print", action="store_true", help="print the generated markdown to stdout")
    parser.add_argument(
        "--strict",
        action="store_true",
        help="exit non-zero when oracle mismatches or result warnings are present",
    )
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv)
    results = load_json(args.json)
    if results is None:
        print("error: result JSON not found: %s" % args.json, file=sys.stderr)
        return 2
    baseline = load_json(args.baseline) if args.baseline else None
    baseline2 = load_json(args.baseline2) if args.baseline2 else None
    if args.baseline2 and baseline2 is None:
        print("error: second baseline JSON not found: %s" % args.baseline2, file=sys.stderr)
        return 2
    label = args.label
    output = render(
        results, baseline, baseline2, args.json, args.baseline if baseline else None, label
    )
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write(output)
    if args.print:
        sys.stdout.write(output)
    print("wrote %s (%d bytes)" % (out_path, len(output.encode("utf-8"))), file=sys.stderr)
    if args.strict:
        problems = []
        if (results.get("oracle_verification") or {}).get("mismatches"):
            problems.append("oracle mismatches")
        if results.get("warnings"):
            problems.append("result warnings")
        if problems:
            print("strict: %s" % ", ".join(problems), file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
