# lsaca cross-language benchmarks

This directory holds the benchmark programs and the measurement harness used for the
"Benchmarks" page of the language manual (docs/language/21-benchmarks.md).

The goal is a *small* but honest comparison: the same workloads implemented
idiomatically in lsaca, C, Rust, Python and Node.js, with every run validated against an
expected checksum so no language can "win" by doing less work. The same source is also
measured at more than one data size: thirteen workloads take arguments and are run at
small, medium and large scales. Four algorithms have a second implementation that uses a
different strategy (iterative and memoised fibonacci, insertion sort, array-based word
counting) so implementation choices can be compared within one language. Expected
checksums are never captured from a run: scripts/oracles.py reimplements every executed
workload in Python and the harness refuses to start when an expected line disagrees with
it. Six functional check programs under benchmarks/programs/checks/ assert closed forms,
sort invariants and other properties outside the measured tables. Three compile-only
workloads measure how the lsaca build scales with source size and project layout: a
generated 4,511-line source, a nine-file multi-module program, and a thirteen-file project
whose binary package depends on two library packages through path dependencies.

## Layout

    benchmarks/
      programs/            five sources per executed workload (lsaca, C, Rust,
                           Python, Node.js), e.g. fib.ls fib.c fib.rs fib.py fib.js;
                           workloads: fib, loop_sum, string_build, vec_memory,
                           sort_i64, matrix_multiply, binary_trees,
                           binary_trees_refs, string_split, sieve, mandelbrot,
                           iterator_pipeline, hashmap_wordcount, generic_stack,
                           fn_pipeline, string_unicode, hashmap_i64,
                           hashmap_string
        fib_iter.*, fib_memo.*, sort_insertion.*, array_wordcount.*
                           alternative implementations, two sources each (lsaca and
                           C only), producing the same checksums as the workload
                           they mirror
        checks/            six functional/property check programs (lsaca only); not
                           part of the measured tables, driven by
                           scripts/bench_checks.py
        compile_large.ls   generated 4,511-line lsaca source (compile-only workload)
        gen_compile_large.py  stdlib-only generator for compile_large.ls
        modules_compile/   compile-only multi-module program: main.ls plus eight
                           sibling module files (about 3,600 lines)
      packages/            compile-only package project: the binary package compile_app
                           plus the library packages util_a and util_b (util_b depends
                           on util_a), thirteen files and about 1,100 lines in total,
                           including three lsaca.toml manifests
      results/
        latest.json        raw measurement data (machine + toolchains + timings)
        latest.md          generated markdown tables (a subset of the JSON)
        v2-docs-page-template.md  layout of the v2.0 report sections for the docs page
        v2-official-run-plan.md   exact command and acceptance checks for the official run
        v2-report-preview.md      dry run of the report sections against the v1.7 archive
        v2-report-sections.md     generated v2.0 report sections (after a run)
        v2.0-report.md     rendered report the docs page was refreshed from
        archive/v1.7/      frozen copy of the published 1.7.0 run (before/after baseline)
        archive/v2.0-pre-perf/  Phase D pre-performance baseline (directional reference)
        archive/v2.0/      frozen copy of the published 2.0.0 run
      README.md            this file

scripts/bench.py is the harness; scripts/oracles.py holds the independent reference
implementations and the small/medium/large scale matrix, and scripts/bench_checks.py
builds and runs the functional check programs. All three are standard-library-only
Python 3 and work on Windows, Linux and macOS. scripts/bench_report.py renders the
v2.0 report sections (workload detail, build artefacts, data scales, independent
oracles and the before/after comparison) from a result JSON plus the frozen v1.7
archive and the Phase D pre-performance archive; it writes
benchmarks/results/v2-report-sections.md after an official run.

## Quick start

Build the compiler once (from the repository root):

    cargo build --release -p lsaca-cli

Then run the suite from the repository root:

    python scripts/bench.py

That takes several minutes on a modern laptop. The suite has twenty-five programs:
twenty-two are executed (7 measured runs each plus the peak-memory run) and three are
compile-only; thirteen of the executed workloads also have small and large data-scale rows
(3 measured runs each by default). The published 1.6 run took about three and a half
minutes for its nineteen programs, eighteen of them executed; the 2.0 suite adds the four
alternative workloads, the data-scale rows and the functional check programs, so a full
run takes noticeably longer. The run writes:

- benchmarks/results/latest.json  (raw data)
- benchmarks/results/latest.md    (tables)
- a markdown summary on stdout

Useful options:

    python scripts/bench.py --quick                    # faster: 3 runtime runs, 1 compile run
    python scripts/bench.py --list                     # show workload/language names
    python scripts/bench.py --list-scales              # scale arguments + expected lines
    python scripts/bench.py --programs fib,vec_memory  # subset of workloads
    python scripts/bench.py --programs sort_i64,compile_large
    python scripts/bench.py --languages lsaca,c        # subset of languages
    python scripts/bench.py --scale small,large        # subset of the data scales
    python scripts/bench.py --scale-runs 5             # runs per small/large scale (default 3)
    python scripts/bench.py --build-mode fallback      # emit-c -r + direct gcc (labelled)
    python scripts/bench.py --help                     # all options

    python scripts/bench_checks.py --debug             # functional check programs

The default (auto) build mode tries the normal `lsaca build -r` path first (the direct C
compiler back end since 1.2) and only falls back to `emit-c -r` plus gcc if that path
fails; fallback results are labelled in the JSON and in the markdown tables.

## Workloads

| name | what it measures | workload | expected stdout |
| --- | --- | --- | --- |
| fib | call overhead / recursion | recursive fib(30), 10 repetitions | checksum=8320400 |
| fib_iter | iterative fibonacci (alternative implementation) | the same fib(30) computed with two rolling values, 10 repetitions; same checksum as fib | checksum=8320400 |
| fib_memo | memoised recursion (alternative implementation) | the same fib(30) computed recursively with a memo table, 10 repetitions; same checksum as fib | checksum=8320400 |
| loop_sum | tight integer loop | sum of 1..=8,000,000 | checksum=32000004000000 |
| string_build | string concatenation | 1,000,000 decimal tokens (i % 1000) each followed by a comma, then length and token sum | checksum=3890000,499500000 |
| vec_memory | allocation / peak memory | 1,000,000 Point{x=i, y=2*i} records, checksum of x+y | checksum=1499998500000 |
| sort_i64 | sorting / indexed writes | in-language iterative quicksort of 200,000 pseudo-random i64 values; checksum is first,last,sum | checksum=21749,2147481261,214813518297651 |
| sort_insertion | insertion sort on a small input (alternative implementation) | the same pseudo-random sequence, n=1,000, sorted with insertion sort instead of quicksort; checksum is first,last,sum | checksum=91125,2140623151,1054904362226 |
| matrix_multiply | nested loops / arithmetic | naive triple-loop multiply of two 160x160 i64 matrices; checksum is the sum of the result | checksum=1018924712000 |
| binary_trees | recursive allocation / pointer chasing | perfect binary tree of depth 16 (131,071 nodes), recursive build and sum; the lsaca traversal takes its input by value and clones the child subtrees explicitly | checksum=2147450880 |
| binary_trees_refs | borrowed traversal (references) | the same tree as binary_trees, but summed through borrowed references (&Option<Box<Node>>) instead of by value, so the traversal copies nothing; checksum matches binary_trees | checksum=2147450880 |
| string_split | string splitting / parsing | build a 194,500-byte comma-separated numeric string, split it, parse every token and sum | checksum=194500,50000,24975000 |
| sieve | array marking / branches | sieve of Eratosthenes, count primes below 2,000,000 | checksum=148933 |
| mandelbrot | floating point math | 600x400 Mandelbrot grid, 100 iterations max, sum of iteration counts | checksum=6282265 |
| iterator_pipeline | lazy iterator pipeline overhead | 1,000,000 i64 values through .iter().filter(even).map(x*x).fold(0, add); checksum is the folded sum | checksum=166666166667000000 |
| hashmap_wordcount | hash map counting with String keys | 200,000 deterministic words from a 4,096-word alphabet counted in a map; checksum is the sum of squared frequencies, the number of distinct keys and the highest frequency | checksum=9766208,4096,49 |
| array_wordcount | flat array vs hash map (alternative implementation) | the same 200,000 words and 4,096 counters as hashmap_wordcount, counted in a flat indexed array instead of a hash map; same checksum | checksum=9766208,4096,49 |
| generic_stack | generic type instantiation | one generic Stack<T> instantiated for i64 (1,000,000 push/pop) and String (1,000 strings), same code both times | checksum=499999500000,5900 |
| fn_pipeline | function values in struct fields, mixed ownership | three Rule{label: String, apply: fn(i64)->i64} records applied 1,000x each plus a clone of the first, 500x; checksum is result sum and label bytes | checksum=53637,22 |
| string_unicode | UTF-8 string handling | build a 120,000-scalar mixed ASCII/Unicode string one char at a time, fold every scalar from iteration and 1,000 random char_at accesses, apply full-Unicode to_uppercase, slice on character boundaries, push/pop and extend; checksum is char count, byte length and the folded scalar checksum | checksum=3357,6921,368454760 |
| hashmap_i64 | hash map with integer keys | 1,000,000 inserts, 1,000,000 present lookups, 1,000,000 absent lookups and 100,000 deletions; checksum is the folded value sum, final length and hit count | checksum=821060469,900000,1000000 |
| hashmap_string | hash map with String keys | 200,000 distinct generated String keys, 200,000 present lookups, 200,000 absent lookups (folding the absent key bytes) and 20,000 deletions; checksum is the folded value sum, final length and hit count | checksum=794025725,180000,200000 |
| compile_large | lsaca build-time scaling (compile-only) | generated 4,511-line lsaca source with 450 functions; built but not executed | checksum=14655037 (not run by the harness) |
| modules_compile | lsaca multi-file build scaling (compile-only) | one crate root (main.ls) plus eight sibling module files, about 3,600 lines covering structs, enums, generics, trait impls, strings, vectors and hash maps; built but not executed | checksum=547598677 (not run by the harness) |
| package_compile | lsaca package-mode build scaling (compile-only) | a thirteen-file, three-package project: the binary package compile_app plus the library packages util_a and util_b, where util_b depends on util_a through a path dependency; built but not executed | checksum=417367897 (not run by the harness) |

Each executed program prints exactly one line (checksum=...) and exits with code 0. The
harness fails the whole run on any mismatch or non-zero exit, so a typo in one
implementation cannot silently skew the tables. The compile-only workloads (compile_large,
modules_compile and package_compile) are the exception: they are built but never executed,
because their purpose is to measure build time for different source and project layouts.

The four alternative implementations (fib_iter, fib_memo, sort_insertion, array_wordcount)
ship lsaca and C sources only: their point is comparing two strategies in the same language
on identical data, and the C version is the same algorithm in C's closest honest style.
Their checksums are identical to the workload they mirror, so the harness still validates
that each implementation does the same work.

sort_i64 uses the same deterministic LCG (16807 * state % 2147483647, seed 12345) in
every language, and the other workloads use fixed integer formulas, so all five
implementations process identical data. Every checksum is an integer (or a list of
integers) so it is bit-exact across languages. compile_large.ls is generated:
run python benchmarks/programs/gen_compile_large.py to recreate it; do not edit it by
hand.

## Data scales

Thirteen workloads take their size from arguments, so the same source is measured at
three data scales instead of one. The medium scale is the primary measurement in the
tables above: the program is run with no arguments and keeps its default size. The small
and large scales pass explicit arguments and are measured with
`--scale-runs` runs (default 3):

| program | small | medium (default) | large |
| --- | --- | --- | --- |
| fib, fib_iter, fib_memo | n=20, 10 repetitions | n=30, 10 repetitions | n=31, 10 repetitions |
| loop_sum | sum 1..=1,000,000 | sum 1..=8,000,000 | sum 1..=12,000,000 |
| string_build | 10,000 tokens | 1,000,000 tokens | 4,000,000 tokens |
| sort_i64 | 1,000 values | 200,000 values | 500,000 values |
| sort_insertion | 250 values | 1,000 values | 4,000 values |
| binary_trees, binary_trees_refs | depth 10 | depth 16 | depth 18 |
| string_split | 1,000 tokens | 50,000 tokens | 500,000 tokens |
| sieve | limit 100,000 | limit 2,000,000 | limit 5,000,000 |
| hashmap_wordcount, array_wordcount | 10,000 tokens | 200,000 tokens | 1,000,000 tokens |

Small sizes keep the extra rows cheap, and the large sizes stay around a few seconds even
in the slower languages. `python scripts/bench.py --list-scales` prints the exact
arguments and the expected output line for every (program, scale) pair, and `--scale`
restricts which scales a run measures. The runtime-by-scale table is written to
latest.md and the scale records live under `programs.<name>.scales` in latest.json, so
the primary tables keep their original shape.

## Independent oracles

Expected checksums are not captured from a run. scripts/oracles.py reimplements the
workload algorithms in plain Python, from the workload descriptions and arithmetic alone,
and is the single source of truth for both the primary and the scale expected lines. The
harness calls it before measuring anything; if a PROGRAMS entry disagrees with the oracle
it aborts with "independent oracle mismatch" instead of recording numbers, so a stale
checksum cannot silently drift. Every language is then validated byte-for-byte against the
same expected line, so a mismatch in the lsaca, C, Rust, Python or Node.js implementation
fails the run before a number is recorded. The verification report is stored under
`oracle_verification` in latest.json and summarised in the generated markdown (number of
program and scale checksums verified, number of mismatches).

## Functional checks

benchmarks/programs/checks/ holds six lsaca-only programs that test behaviour instead of
speed:

- checks_loop_sum.ls: the closed-form sum n*(n+1)/2 at many sizes, then 1,000 repetitions.
- checks_loop_sum_scale.ls: the same property at three argument-driven data scales.
- checks_sort_invariants.ls: a hand-written insertion sort over 2,000 values; the output
  must be ordered, keep the length and the sum, and expose the correct minimum and maximum.
- checks_fib_relations.ls: recursive, iterative and memoised fibonacci must agree for
  n=0..20 and a known value must hold at n=50.
- checks_hashmap_counts.ls: hash-map counts must agree with a flat counter array, including
  the sum of squared frequencies over the key set.
- checks_string_roundtrip.ls: split/parse/format round-trips, char counts, case conversion
  and prefix/suffix slicing.

They self-assert with assert!/assert_eq!, so a wrong result exits nonzero, and each prints
one "ok ..." line. scripts/bench_checks.py rebuilds them, runs every argument set, and
compares the output byte for byte; it also requires empty stderr, which catches leaked
runtime allocations in debug builds:

    python scripts/bench_checks.py --debug              # release and debug builds
    python scripts/bench_checks.py --warnings-as-errors
    python scripts/bench_checks.py --list               # cases and expected lines

The scale checksums are recomputed by the script's own Python reference, never read from a
run. CI runs the quick check set in the cross-platform matrix workflow and the full set in
the weekly smoke workflow.

## Statistics

The runtime tables report the median and the minimum of the measured runs. latest.json
also records every individual run plus mean_s, stdev_s, an approximated 95% confidence
interval ci95_s (1.96 * stdev / sqrt(n)), and outliers_s (runs farther than 3 * MAD from
the median) with outlier_count, so a noisy host can be spotted afterwards. Data-scale rows
use --scale-runs measured runs (default 3). Tables never mix machines: all rows in one
result file are measured on the same host, and a saved result records that host, so a
result file is never a cross-machine comparison. The cross-platform workflows only check
that the workloads build and produce the expected checksums on each platform.

## What is measured

For every available (workload, language) pair:

1. Compile time:
   - lsaca: the normal build path - lsaca build -r with a fresh LSACA_CACHE_DIR. Since
     1.2 the default back end is the compiler's built-in direct C compiler back end: it
     emits the generated C and drives gcc itself (no CMake; the old CMake + Ninja path
     remains available as --cmake but the harness does not measure it). The cached build
     (second invocation on a warm cache) is measured separately. Cold and cached numbers
     are medians of --compile-repeats runs (default 3); latest.json records every cold and
     cached run plus the minimum and maximum.
   - C: gcc -O2 (one translation unit, no -lm needed).
   - Rust: rustc -O.
   - Python / Node.js: n/a, these are interpreted (Node.js loads and compiles at start-up;
     that start-up cost is included in the runtime numbers instead).
2. Runtime: 1 warm-up run (not measured) plus 7 measured runs; median and minimum are
   reported. The harness records every measured run, and latest.json also carries the
   distribution summary described under Statistics. Data-scale rows run the same
   executable with the scale arguments and use --scale-runs measured runs (no extra
   warm-up).
3. Peak memory: a separate run is sampled for the peak working set of the child process
   (Windows: PROCESS_MEMORY_COUNTERS via psapi; Linux: /usr/bin/time -v, macOS:
   /usr/bin/time -l, with a /proc fallback). This is peak resident memory, not allocated
   bytes, so it includes the language runtime itself. Data-scale rows keep the same
   treatment.
4. Artifact sizes: latest.json records the executable bytes for lsaca, C and Rust
   (exe_bytes) and the generated C size for lsaca (generated_c_bytes, summed over all
   translation units); interpreted languages record source_bytes. The generated markdown
   shows an "Artifact sizes" table.

Machine and toolchain information (CPU, RAM, OS, compiler versions) is collected at run
time and stored in latest.json, so a saved result is self-describing.

## Idiomatic implementations (fairness notes)

The point is comparable work, not identical assembly. Where a language has no close
equivalent for a construct, the program uses that language's normal idiom, and the docs
page describes the differences:

- lsaca: Vec<...> with Vec::with_capacity and push; strings are built with
  push_str(format!("{},", v)). Since 1.4 values move by default: Vec, Box, String, Rc and
  user types that contain them (directly or through a field) are move-only, implicit
  copying is limited to Copy types (integers, floats, bool, and aggregates of Copy parts),
  and a copy has to be requested explicitly with .clone(). 1.2 removed the accidental copies
  in the common paths (binding a Vec::with_capacity temporary and iterating with `for`
  no longer duplicate the buffer), which is why vec_memory is at parity with C. The language
  has had references (`&T` / `&mut T`) with borrow checking since 1.3; 1.4 adds ownership
  and move semantics on top. binary_trees_refs uses `&Option<Box<Node>>` to traverse the
  tree without copying it, while binary_trees (traversal by value) asks for the deep copy it
  measures explicitly with .clone(). Since 1.5 the language has user generics (generic
  structs, enums, functions and impls, monomorphised at compile time), traits, lazy iterator
  adapters (map/filter/take/skip/enumerate/fold/sum/count/any/all/find/collect) and a builtin
  HashMap<K, V>; Vec::len() returns i64. lsaca still has no sort method on Vec, which is why
  sort_i64 keeps its hand-written quicksort.
- C: a manually resized char buffer for string_build, a malloc'ed array of structs for
  vec_memory.
- Rust: String::with_capacity plus push, Vec::with_capacity plus push, to_string().
- Python: list of str plus "".join, list of tuples.
- Node.js: array plus join, array of objects.

The newer workloads add these language-specific choices, all described on the docs page:

- sort_i64: no implementation uses a built-in/library sort even where one exists, because
  that would compare standard libraries instead of the language's own loop performance.
  Every version hand-writes the same iterative quicksort with an explicit stack. The
  lsaca version keeps the sort loop in main (the workload was written before 1.3 added
  reference arguments; the loop uses only indexing and swaps either way).
- matrix_multiply: every language uses one flat row-major array plus index arithmetic
  (no nested vectors/objects), which is the natural representation in lsaca and keeps the
  triple loop identical everywhere.
- binary_trees: lsaca uses Option<Box<Node>>, C uses malloc/free pointers, Rust uses Box,
  Python a __slots__ class and Node.js plain objects. The lsaca traversal takes the subtree
  by value, and since 1.4 that means it consumes it: sum_tree clones the two child subtrees
  with .clone() at every level, so the row pays for the boxed nodes being copied - that is
  the measured cost of the by-value traversal, not a compiler bug. (Since 1.6 the
  Some(boxed) match binding moves the box out of the Option instead of copying it, so the
  remaining copies are the explicit child .clone() calls plus boxed.get(), which reads the
  payload by value.) binary_trees_refs starts
  from the same tree but sums it through a borrowed `&Option<Box<Node>>` parameter, the
  shape a Rust user would write; the difference between the two lsaca rows is what
  references buy (the ownership difference is explicit since 1.4).
- string_split: C scans the buffer and parses with strtoll, Rust/Python/Node split and
  parse, lsaca uses String::split plus parse_i64.
- sieve: every language marks a flat flag array with 64-bit-wide slots (Node.js
  Float64Array, Python list of ints, C/Rust/lsaca i64 vectors) instead of a bit-packed
  sieve, so the memory-access pattern is comparable; the checksum is the prime count.
- mandelbrot: the same f64 escape-time formula everywhere; the checksum is the integer sum
  of iteration counts, so floating-point rounding cannot change the result.
- iterator_pipeline: one lazy chain, filter(even).map(x * x).fold(0, add), over
  1,000,000 i64 values. The lsaca and Rust versions compose real iterator adapters and
  allocate only the source Vec; Python uses a generator expression; Node.js uses an eager
  loop (an array filter/map would allocate two intermediate arrays); C writes the plain
  eager loop. The row isolates the cost of the lazy adapter abstraction against a
  hand-written loop.
- hashmap_wordcount: a counting stress test: 200,000 insert/lookup operations over 4,096
  distinct String keys, checked by the sum of squared frequencies (order-independent), the
  number of distinct keys and the highest frequency. lsaca uses the builtin
  HashMap<String, i64>; Rust uses std::collections::HashMap, Python a dict, Node.js a Map,
  and C a hand-written open-addressing table with FNV-1a hashing and strcmp because C has
  no map in its standard library. Since 1.6 the builtin map is a Swiss-table layout with a
  7/8 load factor: absent lookups, removal and iteration are faster than the 1.5 linear
  probing layout, while present lookups pay for a dependent control-byte-to-key load; on
  this mixed workload the two effects roughly cancel.
- generic_stack: the lsaca stack is a generic struct Stack<T> instantiated for i64 and String
  from the same source, and the compiler monomorphises both; Rust does the same with a generic
  struct, Python and Node.js rely on dynamic typing, and C spells the two instantiations out
  because it has no generics.
- fn_pipeline: the lsaca Rule record holds a non-Copy label String and a Copy fn(i64) -> i64
  handle in one struct; run() borrows the rule (&Rule) and calls (rule.apply)(value), and the
  cloned rule shares nothing but its label copy (fn values are refcounted handles, so copying
  one does not allocate). Rust stores a fn pointer in the struct, C stores a function pointer,
  and Python/Node.js store closures in the record. This exercises fn values inside aggregates,
  which 1.5 made valid C for the first time.
- string_unicode: the lsaca version drives the 1.6 UTF-8 String API directly (push(char),
  chars(), char_at(), is_char_boundary(), substring(), to_uppercase(), pop(), push_str(),
  char_count()). C has hand-written UTF-8 encode/decode helpers and a small uppercase table
  covering the 24-scalar alphabet, Rust uses char, Vec<char> and String::to_uppercase,
  Python 3 strings, and Node.js iterates code points with String.fromCodePoint/toUpperCase.
  The 1,000 char_at probes sample character indices spread across the first 7,000 characters:
  Python and Rust index a code-point array in O(1), C folds the sampled scalars during one
  linear scan, and lsaca calls char_at(probe * 7), which decodes UTF-8 from the start of the
  string up to the requested character.
- hashmap_i64: lsaca uses the builtin HashMap<i64, i64> (a Swiss-table layout since 1.6),
  Rust std::collections::HashMap, Python a dict, Node.js a Map, and C a hand-written
  open-addressing table with splitmix64 hashing, tombstone reuse and 0.5 load-factor growth
  (C has no map in its standard library). All versions do the same four phases.
- hashmap_string: the same shapes with 200,000 generated String keys. The lsaca version
  inserts owned strings into the builtin HashMap<String, i64>, C strdup's its keys and frees
  them, and the other languages use their native string-keyed maps; the absent-lookup phase
  folds the UTF-8 bytes of each absent key in every language.
- fib_iter and fib_memo: the same fib(30) as fib, computed iteratively and with a memo
  table. fib measures call overhead; the two alternatives show that one language can pay
  very different costs for the same result, and that the checksum does not depend on the
  algorithm.
- sort_insertion: insertion sort of 1,000 values next to sort_i64's 200,000-value
  quicksort over the same deterministic LCG sequence. The pair shows the algorithm cost
  dominating at small n.
- array_wordcount: the same word stream as hashmap_wordcount counted in a flat i64 array
  indexed by token % 4,096 instead of a hash map. It separates hash-map overhead from the
  counting loop itself, and it is the honest choice when the key space is small and dense.

A contiguous array of values (C, Rust, lsaca) and a per-element object (Python, Node.js)
are both the natural representation in their language; the compiler/VM overhead that
follows from each choice is part of what the numbers show. Since 1.4 an lsaca program
copies only where it asks for a copy: the explicit .clone() in binary_trees and the single
Rule clone in fn_pipeline are the only copies requested by these sources. Aggregate
literals (struct, tuple, vector, Some/Ok/Err) move their operands into place, and every other pass, traversal, call and return moves or
borrows a value instead of deep-copying it, so the other rows no longer carry the
implicit-copy overhead that 1.3's value semantics had; the docs page explains the measured
effect.

## lsaca build modes (important)

The harness supports three ways to build an lsaca program:

- official: lsaca build -r. That is the real user-facing path. Since 1.2 the default back
  end is the compiler's built-in direct C compiler back end: it emits the generated C,
  compiles it together with the cached runtime translation units using gcc, and links -
  no CMake. The older CMake + Ninja pipeline is still available as
  lsaca build -r --cmake, but the harness does not measure it. This mode needs a working C
  compiler on PATH (gcc in the usual MSYS2 location is found automatically). Since 1.6 a
  large program is split into main.c plus extra translation units so gcc can compile them
  in parallel; compile_large exercises that path.
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
   single line checksum=... . Every expected line has to be recomputable, so also add an
   independent Python implementation of the workload to scripts/oracles.py.
2. Add the name, expected stdout, description and workload text to PROGRAMS in
   scripts/bench.py. Optional keys exist for special workloads:
   - "languages": ["lsaca"] restricts a workload to some languages (default: all five);
   - "compile_only": True builds the program but never executes it, and keeps it out of
     the runtime and peak-memory tables (compile_large uses both keys);
   - "alternative": True marks a second implementation of an existing algorithm; set
     "languages" to the ones you actually wrote (the four existing alternatives ship
     lsaca and C sources).
   Workloads that take a size argument should also get a SCALES entry in
   scripts/oracles.py; the harness then attaches the small/medium/large matrix and
   measures the scales automatically.
3. Run python scripts/bench.py --programs <name> --quick to smoke-test it; the run first
   checks every expected line against scripts/oracles.py. Use
   python scripts/bench.py --list-scales to verify the scale arguments and expected lines.
4. Behaviour/property checks belong in benchmarks/programs/checks/ and are registered in
   CHECKS in scripts/bench_checks.py, with their expected line recomputed there.

## Results and the docs page

A compile-only workload shows up only in the compile-time table, with its name marked
"(compile-only)"; there is no runtime or peak-memory entry for it.

benchmarks/results/latest.json and latest.md are regenerated by every run. The manual page
[docs/language/21-benchmarks.md](../docs/language/21-benchmarks.md) quotes the numbers
measured on the reference machine; those tables are generated from
benchmarks/results/latest.md, and the page explains how to reproduce them. The recorded
run used the `official` build mode (the normal `lsaca build -r` path, not the fallback);
its JSON also contains every individual run, the compiler SHA-256 prefix and the
repository commit, so a saved result is self-describing. The published 2.2.0 run was
recorded on 2026-11-07T17:21:34+08:00 with lsaca 2.2.0 (compiler SHA-256 prefix
1c74d15ecf9a1b49, repository commit 97280dd); latest.json and latest.md hold that run,
and a frozen copy is kept in benchmarks/results/archive/v2.2/. Frozen references are
kept for before/after comparisons: the published 2.1.0 run in
benchmarks/results/archive/v2.1/ (recorded 2026-11-06T18:46:10+08:00, compiler SHA-256
prefix 90fca3bbe4e3ec5e, commit 151a9c2), the published 2.0.0 run in
benchmarks/results/archive/v2.0/ (recorded 2026-11-03T00:20:17+08:00, compiler SHA-256
prefix 2a60c5ac7c9fd34c, commit b151d1a), the published 1.7.0 run in
benchmarks/results/archive/v1.7/ (recorded 2026-11-01T13:25:52+08:00, compiler SHA-256
prefix 37554c98f102841f, commit fd3657e), and the Phase D pre-performance baseline in
benchmarks/results/archive/v2.0-pre-perf/ (taken earlier in the 2.0 cycle on a busier
host, so it is a directional reference only). No archive is overwritten when a new run
replaces latest.json/latest.md. benchmarks/results/v2.2-report.md is the rendered report
for the 2.2.0 run; benchmarks/results/v2.1-report.md and v2.0-report.md are the rendered
reports for those runs, v2-report-preview.md is a dry run of scripts/bench_report.py, and
v2-docs-page-template.md records the layout of the sections the page needs. The top-level
`warnings` list records anything that affects interpretation, for example a single sample
that had to be re-measured because of unrelated background load on the machine. If you
rerun the suite, do not compare fallback numbers with the numbers on the docs page - the
two measure different build pipelines.

All 2.0 additions to latest.json are additive, so schema_version stays 1: settings gains
scales and scale_runs, each executed program gains a scales object, runtime records gain
mean_s/stdev_s/ci95_s/outliers_s/outlier_count, compile records gain exe_bytes /
generated_c_bytes / source_bytes, and a top-level oracle_verification report is added. The
original tables keep their headings, row order and cell format, which keeps
scripts/check_bench21.py working against the docs page.

Two CI workflows exercise the suite without publishing performance numbers.
.github/workflows/benchmark-smoke.yml runs the functional check programs and a quick
benchmark run on Ubuntu every week. .github/workflows/benchmark-matrix.yml runs the check
programs and a small benchmark subset on ubuntu-latest, macos-latest and windows-latest and
uploads one artifact per platform. Those jobs verify that every workload builds and
produces the expected checksum on each platform; they are not a cross-machine speed
comparison, and their results are not merged into latest.json. Both jobs pass --quick,
which caps compile repeats at 1, so they stay fast; when a CI-style run needs cold-build
medians rather than a single cold sample, pass --compile-repeats 2. The published
official run keeps the default of three cold builds.
