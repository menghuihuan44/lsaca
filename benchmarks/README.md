# lsaca cross-language benchmarks

This directory holds the benchmark programs and the measurement harness used for the
"Benchmarks" page of the language manual (docs/language/21-benchmarks.md).

The goal is a *small* but honest comparison: the same workloads implemented
idiomatically in lsaca, C, Rust, Python and Node.js, with every run validated against an
expected checksum so no language can "win" by doing less work. One generated
compile-only workload measures how the lsaca build scales with source size.

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

That takes a few minutes on a modern laptop (the published 1.6 run took about three and
a half minutes for nineteen programs, eighteen of them executed with 7 runs each, plus the
peak-memory runs) and writes:

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
| binary_trees | recursive allocation / pointer chasing | perfect binary tree of depth 16 (131,071 nodes), recursive build and sum; the lsaca traversal takes its input by value and clones the child subtrees explicitly | checksum=2147450880 |
| binary_trees_refs | borrowed traversal (references) | the same tree as binary_trees, but summed through borrowed references (&Option<Box<Node>>) instead of by value, so the traversal copies nothing; checksum matches binary_trees | checksum=2147450880 |
| string_split | string splitting / parsing | build a 194,500-byte comma-separated numeric string, split it, parse every token and sum | checksum=194500,50000,24975000 |
| sieve | array marking / branches | sieve of Eratosthenes, count primes below 2,000,000 | checksum=148933 |
| mandelbrot | floating point math | 600x400 Mandelbrot grid, 100 iterations max, sum of iteration counts | checksum=6282265 |
| iterator_pipeline | lazy iterator pipeline overhead | 1,000,000 i64 values through .iter().filter(even).map(x*x).fold(0, add); checksum is the folded sum | checksum=166666166667000000 |
| hashmap_wordcount | hash map counting with String keys | 200,000 deterministic words from a 4,096-word alphabet counted in a map; checksum is the sum of squared frequencies, the number of distinct keys and the highest frequency | checksum=9766208,4096,49 |
| generic_stack | generic type instantiation | one generic Stack<T> instantiated for i64 (1,000,000 push/pop) and String (1,000 strings), same code both times | checksum=499999500000,5900 |
| fn_pipeline | function values in struct fields, mixed ownership | three Rule{label: String, apply: fn(i64)->i64} records applied 1,000x each plus a clone of the first, 500x; checksum is result sum and label bytes | checksum=53637,22 |
| string_unicode | UTF-8 string handling | build a 120,000-scalar mixed ASCII/Unicode string one char at a time, fold every scalar from iteration and 1,000 random char_at accesses, apply full-Unicode to_uppercase, slice on character boundaries, push/pop and extend; checksum is char count, byte length and the folded scalar checksum | checksum=3357,6921,368454760 |
| hashmap_i64 | hash map with integer keys | 1,000,000 inserts, 1,000,000 present lookups, 1,000,000 absent lookups and 100,000 deletions; checksum is the folded value sum, final length and hit count | checksum=821060469,900000,1000000 |
| hashmap_string | hash map with String keys | 200,000 distinct generated String keys, 200,000 present lookups, 200,000 absent lookups (folding the absent key bytes) and 20,000 deletions; checksum is the folded value sum, final length and hit count | checksum=794025725,180000,200000 |
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
[docs/language/21-benchmarks.md](../docs/language/21-benchmarks.md) quotes the numbers
measured on the reference machine; those tables are generated from
benchmarks/results/latest.md, and the page explains how to reproduce them. The recorded
run used the `official` build mode (the normal `lsaca build -r` path, not the fallback);
its JSON also contains every individual run, the compiler SHA-256 prefix and the
repository commit, so a saved result is self-describing. The published v1.6 run was
recorded on 2026-10-31T23:24:44+08:00 with lsaca 1.6.0 (compiler SHA-256 prefix
b1489a3dec37e163, repository commit 59d56c7). The top-level `warnings` list
records anything that affects interpretation, for example a single sample that had to be
re-measured because of unrelated background load on the machine. If you rerun the suite,
do not compare fallback numbers with the numbers on the docs page - the two measure
different build pipelines.
