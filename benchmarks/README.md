# lsaca cross-language benchmarks

This directory holds the benchmark programs and the measurement harness used for the
"Benchmarks" page of the language manual (docs/language/15-benchmarks.md).

The goal is a *small* but honest comparison: the same four workloads implemented
idiomatically in lsaca, C, Rust, Python and Node.js, with every run validated against an
expected checksum so no language can "win" by doing less work.

## Layout

    benchmarks/
      programs/            one source per (workload, language)
        fib.ls  fib.c  fib.rs  fib.py  fib.js
        loop_sum.ls  ...
        string_build.ls  ...
        vec_memory.ls  ...
      results/
        latest.json        raw measurement data (machine + toolchains + timings)
        latest.md          generated markdown tables (a subset of the JSON)
      README.md            this file

scripts/bench.py is the harness. It is standard-library-only Python 3 and works on
Windows, Linux and macOS.

## Quick start

Build the compiler once (from the repository root):

    cargo build --release -p lsaca-cli

Then run the suite from the repository root:

    python scripts/bench.py

That takes roughly 3-5 minutes on a modern laptop and writes:

- benchmarks/results/latest.json  (raw data)
- benchmarks/results/latest.md    (tables)
- a markdown summary on stdout

Useful options:

    python scripts/bench.py --quick                    # ~1 min: 3 runtime runs, 1 compile run
    python scripts/bench.py --list                     # show workload/language names
    python scripts/bench.py --programs fib,vec_memory  # subset of workloads
    python scripts/bench.py --languages lsaca,c        # subset of languages
    python scripts/bench.py --build-mode fallback      # skip CMake: emit-c + direct gcc
    python scripts/bench.py --help                     # all options

The default (auto) build mode tries the official CMake path first and only falls back to
emit-c + gcc if that path fails; fallback results are labelled in the JSON and in the
markdown tables.

## Workloads

| name | what it measures | workload | expected stdout |
| --- | --- | --- | --- |
| fib | call overhead / recursion | recursive fib(30), 10 repetitions | checksum=8320400 |
| loop_sum | tight integer loop | sum of 1..=8,000,000 | checksum=32000004000000 |
| string_build | string concatenation | 1,000,000 decimal tokens (i % 1000) each followed by a comma, then length and token sum | checksum=3890000,499500000 |
| vec_memory | allocation / peak memory | 1,000,000 Point{x=i, y=2*i} records, checksum of x+y | checksum=1499998500000 |

Each program prints exactly one line (checksum=...) and exits with code 0. The harness
fails the whole run on any mismatch or non-zero exit, so a typo in one implementation
cannot silently skew the tables.

## What is measured

For every available (workload, language) pair:

1. Compile time:
   - lsaca: the official build path - lsaca build -r with a fresh LSACA_CACHE_DIR
     (that includes CMake configure plus gcc). The cached build (second invocation on a
     warm cache) is measured separately.
   - C: gcc -O2 (one translation unit, no -lm needed).
   - Rust: rustc -O.
   - Python / Node.js: n/a, these are interpreted (Node.js loads and compiles at start-up;
     that start-up cost is included in the runtime numbers instead).
2. Runtime: 1 warm-up run (not measured) plus 7 measured runs; median and minimum are
   reported. The harness records every measured run.
3. Peak memory: a separate run is sampled for the peak working set of the child process
   (Windows: PROCESS_MEMORY_COUNTERS via psapi; Linux: /usr/bin/time -v, macOS:
   /usr/bin/time -l, with a /proc fallback). This is peak resident memory, not allocated
   bytes, so it includes the language runtime itself.

Machine and toolchain information (CPU, RAM, OS, compiler versions) is collected at run
time and stored in latest.json, so a saved result is self-describing.

## Idiomatic implementations (fairness notes)

The point is comparable work, not identical assembly. Where a language has no close
equivalent for a construct, the program uses that language's normal idiom, and the docs
page describes the differences:

- lsaca: Vec<...> with Vec::with_capacity and push; strings are built with
  push_str(format!("{},", v)). An lsaca Vec is a value type, so assigning it or iterating
  it with `for` deep-copies the buffer - that cost is visible in the vec_memory numbers
  and peak memory. lsaca has no sort method on Vec and no generics, which is why the
  workloads stay simple.
- C: a manually resized char buffer for string_build, a malloc'ed array of structs for
  vec_memory.
- Rust: String::with_capacity plus push, Vec::with_capacity plus push, to_string().
- Python: list of str plus "".join, list of tuples.
- Node.js: array plus join, array of objects.

A contiguous array of values (C, Rust, lsaca) and a per-element object (Python, Node.js)
are both the natural representation in their language; the compiler/VM overhead that
follows from each choice is part of what the numbers show. In addition, lsaca copies
vectors by value (no references in the language yet), so the vec_memory row pays for two
extra deep copies of the 16 MB buffer; the docs page explains the measured effect.

## lsaca build modes (important)

The harness supports three ways to build an lsaca program:

- official: lsaca build -r. That is the real user-facing path: emit C,
  CMake configure, CMake build. It needs a working cmake and a C compiler on PATH.
- fallback: lsaca emit-c followed by a direct gcc -O2 of the generated C. This is not the
  official path, but it is enough to compare generated-C performance, and it works in
  environments where nested CMake/ninja child processes are blocked.
- auto (default): try official first; on the first failure or timeout, record the error in
  latest.json and use the fallback for the remaining programs. Every table produced by a
  fallback run is labelled as such.

If the official build is unusable on your machine, the fallback columns are shown
explicitly (headers read "lsaca emit-c + gcc") and the official cold/cached columns are
n/a. Do not compare fallback compile-time columns to official compile-time columns: they
measure different pipelines.

The harness uses a fresh LSACA_CACHE_DIR (under target/bench-tmp/cache) for cold-build
measurements, and the same directory again for the cached-build measurement, so the two
numbers are cleanly separated. It also sets TEMP/TMP/TMPDIR to a directory inside
target/bench-tmp and prepends the usual tool directories (C:\msys64\ucrt64\bin,
~/.cargo/bin, C:\Program Files\CMake\bin) to PATH, so the result does not depend on
the caller's shell configuration.

## Adding a workload

1. Add benchmarks/programs/<name>.<ext> for each language you want to cover, printing a
   single line checksum=...
2. Add the name, expected stdout, description and workload text to PROGRAMS in
   scripts/bench.py.
3. Run python scripts/bench.py --programs <name> --quick to smoke-test it.

## Results and the docs page

benchmarks/results/latest.json and latest.md are regenerated by every run. The manual page
[docs/language/15-benchmarks.md](../docs/language/15-benchmarks.md) quotes the numbers
measured on the reference machine; those tables are generated from
benchmarks/results/latest.md, and the page explains how to reproduce them. The recorded
run used the official build mode; its JSON also contains every individual run, the
compiler SHA-256 prefix and the repository commit, so a saved result is self-describing.
If you rerun the suite, do not compare fallback numbers with the official numbers in the
docs page - the two measure different build pipelines.
