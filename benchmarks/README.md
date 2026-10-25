# lsaca cross-language benchmarks

This directory holds the benchmark programs and the measurement harness used for the
"Benchmarks" page of the language manual (docs/language/15-benchmarks.md).

The goal is a *small* but honest comparison: the same workloads implemented
idiomatically in lsaca, C, Rust, Python and Node.js, with every run validated against an
expected checksum so no language can "win" by doing less work. One generated
compile-only workload measures how the lsaca build scales with source size.

## Layout

    benchmarks/
      programs/            one source per (workload, language)
        fib.ls  fib.c  fib.rs  fib.py  fib.js
        loop_sum.ls  ...            (and the same for: string_build, vec_memory,
        sort_i64.ls  ...             matrix_multiply, binary_trees, string_split,
        sieve.ls  ...                sieve, mandelbrot)
        compile_large.ls   generated 4,511-line lsaca source (compile-only workload)
        gen_compile_large.py  stdlib-only generator for compile_large.ls
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

That takes roughly 3-5 minutes on a modern laptop (eleven programs, ten of them executed
with 7 runs each, plus the peak-memory runs) and writes:

- benchmarks/results/latest.json  (raw data)
- benchmarks/results/latest.md    (tables)
- a markdown summary on stdout

Useful options:

    python scripts/bench.py --quick                    # faster: 3 runtime runs, 1 compile run
    python scripts/bench.py --list                     # show workload/language names
    python scripts/bench.py --programs fib,vec_memory  # subset of workloads
    python scripts/bench.py --programs sort_i64,compile_large
    python scripts/bench.py --languages lsaca,c        # subset of languages
    python scripts/bench.py --build-mode fallback      # emit-c -r + direct gcc (labelled)
    python scripts/bench.py --help                     # all options

The default (auto) build mode tries the normal `lsaca build -r` path first (the direct C
compiler back end since 1.2) and only falls back to `emit-c -r` plus gcc if that path
fails; fallback results are labelled in the JSON and in the markdown tables.

## Workloads

| name | what it measures | workload | expected stdout |
| --- | --- | --- | --- |
| fib | call overhead / recursion | recursive fib(30), 10 repetitions | checksum=8320400 |
| loop_sum | tight integer loop | sum of 1..=8,000,000 | checksum=32000004000000 |
| string_build | string concatenation | 1,000,000 decimal tokens (i % 1000) each followed by a comma, then length and token sum | checksum=3890000,499500000 |
| vec_memory | allocation / peak memory | 1,000,000 Point{x=i, y=2*i} records, checksum of x+y | checksum=1499998500000 |
| sort_i64 | sorting / indexed writes | in-language iterative quicksort of 200,000 pseudo-random i64 values; checksum is first,last,sum | checksum=21749,2147481261,214813518297651 |
| matrix_multiply | nested loops / arithmetic | naive triple-loop multiply of two 160x160 i64 matrices; checksum is the sum of the result | checksum=1018924712000 |
| binary_trees | recursive allocation / pointer chasing | perfect binary tree of depth 16 (131,071 nodes), recursive build and sum | checksum=2147450880 |
| string_split | string splitting / parsing | build a 194,500-byte comma-separated numeric string, split it, parse every token and sum | checksum=194500,50000,24975000 |
| sieve | array marking / branches | sieve of Eratosthenes, count primes below 2,000,000 | checksum=148933 |
| mandelbrot | floating point math | 600x400 Mandelbrot grid, 100 iterations max, sum of iteration counts | checksum=6282265 |
| compile_large | lsaca build-time scaling (compile-only) | generated 4,511-line lsaca source with 450 functions; built but not executed | checksum=14655037 (not run by the harness) |

Each executed program prints exactly one line (checksum=...) and exits with code 0. The
harness fails the whole run on any mismatch or non-zero exit, so a typo in one
implementation cannot silently skew the tables. compile_large is the exception: it is
built but never executed, because its purpose is to measure build time.

sort_i64 uses the same deterministic LCG (16807 * state % 2147483647, seed 12345) in
every language, and the other workloads use fixed integer formulas, so all five
implementations process identical data. Every checksum is an integer (or a list of
integers) so it is bit-exact across languages. compile_large.ls is generated:
run python benchmarks/programs/gen_compile_large.py to recreate it; do not edit it by
hand.

## What is measured

For every available (workload, language) pair:

1. Compile time:
   - lsaca: the normal build path - lsaca build -r with a fresh LSACA_CACHE_DIR. Since
     1.2 the default back end is the compiler's built-in direct C compiler back end: it
     emits the generated C and drives gcc itself (no CMake; the old CMake + Ninja path
     remains available as --cmake but the harness does not measure it). The cached build
     (second invocation on a warm cache) is measured separately.
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
  push_str(format!("{},", v)). An lsaca Vec is still a value type (the language has no
  references yet), but 1.2 avoids the physical copies in the common paths: binding a
  Vec::with_capacity temporary and iterating with `for` no longer duplicate the buffer,
  which is why vec_memory is at parity with C. Recursive Box payloads still deep-copy on
  get(); that is the binary_trees cost. lsaca has no sort method on Vec and no generics,
  which is why the workloads stay simple.
- C: a manually resized char buffer for string_build, a malloc'ed array of structs for
  vec_memory.
- Rust: String::with_capacity plus push, Vec::with_capacity plus push, to_string().
- Python: list of str plus "".join, list of tuples.
- Node.js: array plus join, array of objects.

The newer workloads add these language-specific choices, all described on the docs page:

- sort_i64: no implementation uses a built-in/library sort even where one exists, because
  that would compare standard libraries instead of the language's own loop performance.
  Every version hand-writes the same iterative quicksort with an explicit stack. The
  lsaca version keeps the sort loop in main because lsaca function arguments are
  immutable, so there is no way to pass a mutable Vec to a helper function.
- matrix_multiply: every language uses one flat row-major array plus index arithmetic
  (no nested vectors/objects), which is the natural representation in lsaca and keeps the
  triple loop identical everywhere.
- binary_trees: lsaca uses Option<Box<Node>> (a Box deep-copies its payload on copy), C
  uses malloc/free pointers, Rust uses Box, Python a __slots__ class and Node.js plain
  objects. The lsaca row therefore also pays for the boxed nodes being copied - that is
  the measured cost of value semantics, not a compiler bug.
- string_split: C scans the buffer and parses with strtoll, Rust/Python/Node split and
  parse, lsaca uses String::split plus parse_i64.
- sieve: every language marks a flat flag array with 64-bit-wide slots (Node.js
  Float64Array, Python list of ints, C/Rust/lsaca i64 vectors) instead of a bit-packed
  sieve, so the memory-access pattern is comparable; the checksum is the prime count.
- mandelbrot: the same f64 escape-time formula everywhere; the checksum is the integer sum
  of iteration counts, so floating-point rounding cannot change the result.

A contiguous array of values (C, Rust, lsaca) and a per-element object (Python, Node.js)
are both the natural representation in their language; the compiler/VM overhead that
follows from each choice is part of what the numbers show. Where lsaca still copies
(explicit Vec/Box copies, recursive Box payloads) that cost is part of the measurement
too; the docs page explains the measured effect.

## lsaca build modes (important)

The harness supports three ways to build an lsaca program:

- official: lsaca build -r. That is the real user-facing path. Since 1.2 the default back
  end is the compiler's built-in direct C compiler back end: it emits the generated C,
  compiles it together with the cached runtime translation units using gcc, and links -
  no CMake. The older CMake + Ninja pipeline is still available as
  lsaca build -r --cmake, but the harness does not measure it. This mode needs a working C
  compiler on PATH (gcc in the usual MSYS2 location is found automatically).
- fallback: lsaca emit-c -r followed by a direct gcc -O2 -DNDEBUG of the generated C and
  the runtime sources. This is not the normal path, but it is enough to compare
  generated-C performance, and it works in environments where the compiler's child gcc
  invocations are blocked. It uses -r so the emitted C has release (wrapping) arithmetic
  and the same optimisations as a default build.
- auto (default): try official first; on the first failure or timeout, record the error in
  latest.json and use the fallback for the remaining programs. Every table produced by a
  fallback run is labelled as such (the compile table then shows an "lsaca emit-c + gcc"
  column instead of cold/cached build columns).

If the normal build is unusable on your machine, the fallback columns are shown
explicitly and the cold/cached columns are n/a. Do not compare fallback compile-time
numbers with normal-build numbers: they measure different pipelines.

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
   scripts/bench.py. Two optional keys exist for special workloads:
   - "languages": ["lsaca"] restricts a workload to some languages (default: all five);
   - "compile_only": True builds the program but never executes it, and keeps it out of
     the runtime and peak-memory tables (compile_large uses both keys).
3. Run python scripts/bench.py --programs <name> --quick to smoke-test it.

## Results and the docs page

A compile-only workload shows up only in the compile-time table, with its name marked
"(compile-only)"; there is no runtime or peak-memory entry for it.

benchmarks/results/latest.json and latest.md are regenerated by every run. The manual page
[docs/language/15-benchmarks.md](../docs/language/15-benchmarks.md) quotes the numbers
measured on the reference machine; those tables are generated from
benchmarks/results/latest.md, and the page explains how to reproduce them. The recorded
run used the `official` build mode (the normal `lsaca build -r` path, not the fallback);
its JSON also contains every individual run, the compiler SHA-256 prefix and the
repository commit, so a saved result is self-describing. The top-level `warnings` list
records anything that affects interpretation, for example a single sample that had to be
re-measured because of unrelated background load on the machine. If you rerun the suite,
do not compare fallback numbers with the numbers on the docs page - the two measure
different build pipelines.
