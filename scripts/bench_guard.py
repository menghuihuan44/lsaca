#!/usr/bin/env python3
"""Incremental speed guard for the lsaca benchmark matrix.

The guard compares a benchmark run produced by scripts/bench.py with a committed
baseline, normally benchmarks/results/archive/v2.5/latest.json.  Two metrics are
checked for the lsaca implementation of every program present in both files:

  runtime       the primary runtime median (and every data scale present in
                both runs), tolerance 10% by default;
  cold compile  the cold build compile median, tolerance 20% by default.

Three comparison modes are available:

  absolute     compare wall-clock medians directly; only meaningful when the
               baseline and the current run were recorded on the same machine;
  normalized   compare the lsaca median divided by the C reference median, so
               a slower CI runner cancels out and only lsaca-relative
               regressions remain;
  auto         absolute when both files name the same host platform, CPU and
               toolchain versions, otherwise normalized (the default).

Usage:
    python scripts/bench_guard.py --self-test
    python scripts/bench_guard.py --results target/bench-matrix/results.json
    python scripts/bench_guard.py --results run.json --baseline old.json ^
        --tolerance 10 --json out.json

Exit status: 0 when no metric exceeds its tolerance, 1 when a regression is
found, 2 for invalid input.  The self-test does not need any repository files.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import sys
import tempfile
from pathlib import Path

DEFAULT_BASELINE = "benchmarks/results/archive/v2.5/latest.json"
DEFAULT_RUNTIME_TOLERANCE = 10.0
DEFAULT_COMPILE_TOLERANCE = 20.0


def parse_percent(text):
    value = str(text).strip()
    if value.endswith("%"):
        value = value[:-1].strip()
    try:
        number = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("not a percentage: %r" % text) from error
    if number < 0:
        raise argparse.ArgumentTypeError("a tolerance cannot be negative")
    return number


def load_json(path):
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def languages(program, scale=None):
    if scale is None:
        return program.get("languages") or {}
    entry = (program.get("scales") or {}).get(scale) or {}
    return entry.get("languages") or {}


def runtime_median(program, language, scale=None):
    record = (languages(program, scale).get(language) or {}).get("runtime") or {}
    return record.get("median_s")


def compile_median(program, language):
    record = (languages(program).get(language) or {}).get("compile") or {}
    return record.get("cold_median_s")


def compile_median_c(program):
    record = (languages(program).get("c") or {}).get("compile") or {}
    return record.get("median_s")


def host_platform(results):
    return (results.get("host") or {}).get("platform")


def host_fingerprint(results):
    """Everything that makes absolute medians comparable across runs.

    A compiler upgrade changes the speed of both the lsaca build and the
    reference builds, so the toolchain versions are part of the fingerprint:
    when they differ, the guard falls back to the C-normalized comparison.
    """

    host = results.get("host") or {}
    toolchains = results.get("toolchains") or {}
    fields = [
        host.get("platform"),
        host.get("cpu"),
        host.get("logical_cpus"),
        toolchains.get("gcc"),
        toolchains.get("rustc"),
        toolchains.get("python"),
        toolchains.get("node"),
        (results.get("lsaca_build") or {}).get("mode"),
    ]
    return tuple("" if value is None else str(value) for value in fields)


def resolve_mode(requested, baseline, results):
    if requested != "auto":
        return requested, ""
    if not host_platform(baseline) or not host_platform(results):
        return (
            "normalized",
            "host platform is missing from one result file; using normalized mode",
        )
    if host_fingerprint(baseline) == host_fingerprint(results):
        return "absolute", ""
    return (
        "normalized",
        "baseline and current measurement environments differ (host or toolchain);"
        " using normalized mode",
    )


def build_comparison(
    program,
    metric,
    baseline_median,
    current_median,
    baseline_reference,
    current_reference,
    mode,
    tolerance,
):
    if mode == "normalized":
        if not baseline_reference or not current_reference:
            return None
        baseline_value = baseline_median / baseline_reference
        current_value = current_median / current_reference
    else:
        baseline_value = baseline_median
        current_value = current_median
    change = (current_value - baseline_value) / baseline_value * 100.0
    return {
        "program": program,
        "metric": metric,
        "mode": mode,
        "baseline": round(baseline_value, 9),
        "current": round(current_value, 9),
        "baseline_median_s": round(baseline_median, 9),
        "current_median_s": round(current_median, 9),
        "baseline_reference_s": (
            round(baseline_reference, 9) if baseline_reference else None
        ),
        "current_reference_s": (
            round(current_reference, 9) if current_reference else None
        ),
        "change_pct": round(change, 3),
        "tolerance_pct": tolerance,
        "ok": change <= tolerance + 1e-9,
    }


def compare_results(
    baseline,
    results,
    runtime_tolerance=DEFAULT_RUNTIME_TOLERANCE,
    compile_tolerance=DEFAULT_COMPILE_TOLERANCE,
    mode="auto",
):
    resolved_mode, mode_note = resolve_mode(mode, baseline, results)
    baseline_programs = baseline.get("programs") or {}
    current_programs = results.get("programs") or {}
    comparisons = []
    warnings = []
    if mode_note:
        warnings.append(mode_note)

    for name in sorted(current_programs):
        current = current_programs[name]
        base = baseline_programs.get(name)
        if base is None:
            warnings.append("program %r has no baseline entry; skipped" % name)
            continue

        def add(metric, base_median, current_median, base_ref, current_ref, tol):
            comparison = build_comparison(
                name,
                metric,
                base_median,
                current_median,
                base_ref,
                current_ref,
                resolved_mode,
                tol,
            )
            if comparison is None:
                warnings.append(
                    "program %r metric %r has no C reference for normalized "
                    "comparison; skipped" % (name, metric)
                )
            else:
                comparisons.append(comparison)

        base_runtime = runtime_median(base, "lsaca")
        current_runtime = runtime_median(current, "lsaca")
        if base_runtime is not None and current_runtime is not None:
            add(
                "runtime",
                base_runtime,
                current_runtime,
                runtime_median(base, "c"),
                runtime_median(current, "c"),
                runtime_tolerance,
            )

        base_scales = base.get("scales") or {}
        current_scales = current.get("scales") or {}
        for scale in sorted(set(base_scales) & set(current_scales)):
            base_scale_runtime = runtime_median(base, "lsaca", scale)
            current_scale_runtime = runtime_median(current, "lsaca", scale)
            if base_scale_runtime is None or current_scale_runtime is None:
                continue
            add(
                "runtime[%s]" % scale,
                base_scale_runtime,
                current_scale_runtime,
                runtime_median(base, "c", scale),
                runtime_median(current, "c", scale),
                runtime_tolerance,
            )

        base_compile = compile_median(base, "lsaca")
        current_compile = compile_median(current, "lsaca")
        if base_compile is not None and current_compile is not None:
            add(
                "cold_compile",
                base_compile,
                current_compile,
                compile_median_c(base),
                compile_median_c(current),
                compile_tolerance,
            )

    failures = [comparison for comparison in comparisons if not comparison["ok"]]
    return {
        "mode": resolved_mode,
        "mode_requested": mode,
        "warnings": warnings,
        "comparisons": comparisons,
        "failures": failures,
    }


def fmt_seconds(value):
    if value >= 1.0:
        return "%.3f s" % value
    return "%.2f ms" % (value * 1000.0)


def describe(comparison):
    if comparison["mode"] == "normalized":
        baseline = "%.3fx" % comparison["baseline"]
        current = "%.3fx" % comparison["current"]
    else:
        baseline = fmt_seconds(comparison["baseline"])
        current = fmt_seconds(comparison["current"])
    return "%s %-20s %s -> %s  %+.1f%% (tolerance %.1f%%)" % (
        comparison["metric"],
        comparison["program"],
        baseline,
        current,
        comparison["change_pct"],
        comparison["tolerance_pct"],
    )


def markdown_report(payload):
    lines = [
        "# Speed guard report",
        "",
        "- baseline: %s" % payload["baseline"],
        "- results: %s" % payload["results"],
        "- mode: %s" % payload["mode"],
        "- runtime tolerance: %.1f%%" % payload["runtime_tolerance_pct"],
        "- cold compile tolerance: %.1f%%" % payload["compile_tolerance_pct"],
        "",
        "| program | metric | baseline | current | change | tolerance | result |",
        "| --- | --- | ---: | ---: | ---: | ---: | --- |",
    ]
    for comparison in payload["comparisons"]:
        if comparison["mode"] == "normalized":
            baseline = "%.3fx" % comparison["baseline"]
            current = "%.3fx" % comparison["current"]
        else:
            baseline = fmt_seconds(comparison["baseline"])
            current = fmt_seconds(comparison["current"])
        lines.append(
            "| %s | %s | %s | %s | %+.1f%% | %.1f%% | %s |"
            % (
                comparison["program"],
                comparison["metric"],
                baseline,
                current,
                comparison["change_pct"],
                comparison["tolerance_pct"],
                "ok" if comparison["ok"] else "**regression**",
            )
        )
    if payload["failures"]:
        lines.extend(["", "## Regressions", ""])
        for comparison in payload["failures"]:
            lines.append("- " + describe(comparison))
    else:
        lines.extend(["", "No metric exceeded its tolerance."])
    lines.append("")
    return "\n".join(lines)


def write_outputs(payload, json_path=None, md_path=None):
    if json_path:
        destination = Path(json_path)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    if md_path:
        destination = Path(md_path)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(markdown_report(payload), encoding="utf-8")


def run_guard(args):
    baseline = load_json(args.baseline)
    results = load_json(args.results)
    outcome = compare_results(
        baseline,
        results,
        runtime_tolerance=args.runtime_tolerance,
        compile_tolerance=args.compile_tolerance,
        mode=args.mode,
    )
    payload = {
        "baseline": str(args.baseline),
        "results": str(args.results),
        "mode": outcome["mode"],
        "mode_requested": outcome["mode_requested"],
        "runtime_tolerance_pct": args.runtime_tolerance,
        "compile_tolerance_pct": args.compile_tolerance,
        "comparisons": outcome["comparisons"],
        "failures": outcome["failures"],
        "warnings": outcome["warnings"],
        "ok": not outcome["failures"],
    }
    write_outputs(payload, args.json, args.md)

    if not payload["comparisons"]:
        for warning in payload["warnings"]:
            print("bench guard: warning: " + warning)
        print(
            "bench guard: no comparable metrics (mode %s); pass --mode absolute "
            "to compare medians without a C reference" % payload["mode"]
        )
        return 2
    for warning in payload["warnings"]:
        print("bench guard: warning: " + warning)
    for comparison in payload["comparisons"]:
        print(
            "%s %s"
            % ("ok      " if comparison["ok"] else "REGRESS ", describe(comparison))
        )
    if payload["failures"]:
        print(
            "bench guard: FAILED (%d regression(s) in %d comparisons, mode %s)"
            % (len(payload["failures"]), len(payload["comparisons"]), payload["mode"])
        )
        return 1
    print(
        "bench guard: OK (%d comparisons, mode %s; runtime <= %.1f%%, "
        "cold compile <= %.1f%%)"
        % (
            len(payload["comparisons"]),
            payload["mode"],
            args.runtime_tolerance,
            args.compile_tolerance,
        )
    )
    return 0


# ---------------------------------------------------------------------------
# Self-test


def _program(runtime=None, compile_time=None, c_runtime=None, c_compile=None, scales=None):
    program = {"languages": {"lsaca": {}, "c": {}}}
    if runtime is not None:
        program["languages"]["lsaca"]["runtime"] = {"median_s": runtime}
    if compile_time is not None:
        program["languages"]["lsaca"]["compile"] = {"cold_median_s": compile_time}
    if c_runtime is not None:
        program["languages"]["c"]["runtime"] = {"median_s": c_runtime}
    if c_compile is not None:
        program["languages"]["c"]["compile"] = {"median_s": c_compile}
    if scales is not None:
        program["scales"] = {}
        for name, (runtime_value, c_runtime_value) in scales.items():
            program["scales"][name] = {
                "languages": {
                    "lsaca": {"runtime": {"median_s": runtime_value}},
                    "c": {"runtime": {"median_s": c_runtime_value}},
                }
            }
    return program


def _results(programs, platform="Test-OS-1"):
    return {"host": {"platform": platform}, "programs": programs}


def _check(condition, message, checks):
    checks.append((bool(condition), message))


def run_self_test():
    checks = []
    base = _results(
        {
            "alpha": _program(
                runtime=0.100,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            ),
            "compile_only": _program(compile_time=2.0, c_compile=0.5),
        }
    )

    same = _results(
        {
            "alpha": _program(
                runtime=0.100,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            ),
            "compile_only": _program(compile_time=2.0, c_compile=0.5),
        }
    )
    outcome = compare_results(base, same, mode="absolute")
    _check(len(outcome["failures"]) == 0, "identical results pass", checks)
    _check(
        len(outcome["comparisons"]) == 4,
        "primary, scale, compile, and compile-only are compared",
        checks,
    )

    runtime_11 = _results(
        {
            "alpha": _program(
                runtime=0.111,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            )
        }
    )
    outcome = compare_results(base, runtime_11, mode="absolute")
    _check(len(outcome["failures"]) == 1, "runtime +11% fails the 10% guard", checks)
    _check(
        outcome["failures"][0]["metric"] == "runtime",
        "runtime failure is reported",
        checks,
    )

    runtime_9 = _results(
        {
            "alpha": _program(
                runtime=0.109,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            )
        }
    )
    _check(
        not compare_results(base, runtime_9, mode="absolute")["failures"],
        "runtime +9% passes",
        checks,
    )

    compile_21 = _results(
        {
            "alpha": _program(
                runtime=0.100,
                compile_time=0.605,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            )
        }
    )
    outcome = compare_results(base, compile_21, mode="absolute")
    _check(
        any(item["metric"] == "cold_compile" for item in outcome["failures"]),
        "cold compile +21% fails the 20% guard",
        checks,
    )

    scale_15 = _results(
        {
            "alpha": _program(
                runtime=0.100,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.0115, 0.005)},
            )
        }
    )
    outcome = compare_results(base, scale_15, mode="absolute")
    _check(
        any(item["metric"] == "runtime[small]" for item in outcome["failures"]),
        "data-scale runtime +15% fails",
        checks,
    )

    override = _results(
        {
            "alpha": _program(
                runtime=0.125,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            )
        }
    )
    _check(
        not compare_results(
            base, override, runtime_tolerance=30.0, mode="absolute"
        )["failures"],
        "--runtime-tolerance 30 accepts +25%",
        checks,
    )

    normalized_current = _results(
        {
            "alpha": _program(
                runtime=0.110,
                compile_time=0.550,
                c_runtime=0.055,
                c_compile=0.110,
                scales={"small": (0.011, 0.0055)},
            )
        },
        platform="Other-OS",
    )
    outcome = compare_results(base, normalized_current, mode="normalized")
    _check(
        outcome["mode"] == "normalized",
        "normalized mode is selected explicitly",
        checks,
    )
    _check(
        len(outcome["failures"]) == 0,
        "uniform slowdown cancels in normalized mode",
        checks,
    )

    outcome = compare_results(base, normalized_current, mode="auto")
    _check(
        outcome["mode"] == "normalized",
        "auto falls back to normalized on host mismatch",
        checks,
    )
    _check(
        len(outcome["failures"]) == 0,
        "auto accepts a uniform slowdown on a different host",
        checks,
    )

    toolchain_current = _results(
        {
            "alpha": _program(
                runtime=0.100,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            )
        }
    )
    toolchain_current["toolchains"] = {"gcc": "gcc (Rev6) 16.2.0"}
    outcome = compare_results(base, toolchain_current, mode="auto")
    _check(
        outcome["mode"] == "normalized",
        "auto falls back to normalized when the toolchain differs",
        checks,
    )

    outcome = compare_results(base, runtime_11, mode="auto")
    _check(
        any(item["metric"] == "runtime" for item in outcome["failures"]),
        "auto still catches an lsaca-only runtime regression",
        checks,
    )

    missing = _results(
        {
            "alpha": _program(
                runtime=0.100,
                compile_time=0.500,
                c_runtime=0.050,
                c_compile=0.100,
                scales={"small": (0.010, 0.005)},
            ),
            "new_program": _program(runtime=1.0, compile_time=1.0),
        }
    )
    outcome = compare_results(base, missing, mode="absolute")
    _check(
        any("new_program" in warning for warning in outcome["warnings"]),
        "program without a baseline entry produces a warning",
        checks,
    )

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        baseline_path = root / "baseline.json"
        results_path = root / "results.json"
        report_path = root / "report.json"
        baseline_path.write_text(json.dumps(base), encoding="utf-8")
        results_path.write_text(json.dumps(same), encoding="utf-8")
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = main(
                [
                    "--results",
                    str(results_path),
                    "--baseline",
                    str(baseline_path),
                    "--mode",
                    "absolute",
                    "--json",
                    str(report_path),
                ]
            )
        _check(code == 0, "clean run exits 0", checks)
        report = json.loads(report_path.read_text(encoding="utf-8"))
        _check(
            report["ok"] and len(report["comparisons"]) == 4,
            "JSON report is written",
            checks,
        )

        failure_path = root / "failure.json"
        failure_path.write_text(json.dumps(runtime_11), encoding="utf-8")
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = main(
                [
                    "--results",
                    str(failure_path),
                    "--baseline",
                    str(baseline_path),
                    "--mode",
                    "absolute",
                ]
            )
        _check(code == 1, "regression run exits 1", checks)

    failed = [message for ok, message in checks if not ok]
    if failed:
        print("bench guard self-test: FAILED")
        for message in failed:
            print("  self-test failure: " + message)
        return 1
    print("bench guard self-test: %d/%d checks passed" % (len(checks), len(checks)))
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Compare benchmark results with a committed baseline."
    )
    parser.add_argument(
        "--results", metavar="PATH", help="benchmark JSON produced by scripts/bench.py"
    )
    parser.add_argument(
        "--baseline",
        metavar="PATH",
        default=DEFAULT_BASELINE,
        help="committed baseline JSON (default: %s)" % DEFAULT_BASELINE,
    )
    parser.add_argument(
        "--mode",
        choices=("auto", "absolute", "normalized"),
        default="auto",
        help="absolute medians, C-normalized ratios, or auto (default: auto)",
    )
    parser.add_argument(
        "--tolerance",
        type=parse_percent,
        default=None,
        metavar="PERCENT",
        help="set both tolerances (percent); default runtime 10%%, compile 20%%",
    )
    parser.add_argument(
        "--runtime-tolerance",
        type=parse_percent,
        default=None,
        metavar="PERCENT",
        help="runtime median tolerance in percent (default: 10)",
    )
    parser.add_argument(
        "--compile-tolerance",
        type=parse_percent,
        default=None,
        metavar="PERCENT",
        help="cold compile median tolerance in percent (default: 20)",
    )
    parser.add_argument("--json", metavar="PATH", help="write a JSON report to PATH")
    parser.add_argument("--md", metavar="PATH", help="write a Markdown report to PATH")
    parser.add_argument(
        "--self-test", action="store_true", help="run the built-in comparison self-test"
    )
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_test()

    runtime_tolerance = (
        args.runtime_tolerance
        if args.runtime_tolerance is not None
        else (args.tolerance if args.tolerance is not None else DEFAULT_RUNTIME_TOLERANCE)
    )
    compile_tolerance = (
        args.compile_tolerance
        if args.compile_tolerance is not None
        else (args.tolerance if args.tolerance is not None else DEFAULT_COMPILE_TOLERANCE)
    )
    args.runtime_tolerance = runtime_tolerance
    args.compile_tolerance = compile_tolerance

    if not args.results:
        parser.error("--results is required unless --self-test is used")

    try:
        return run_guard(args)
    except (OSError, ValueError, ZeroDivisionError) as error:
        print("bench guard: invalid input: %s" % error)
        return 2


if __name__ == "__main__":
    sys.exit(main())
