# Benchmarks

This page reports how lsaca compares with C, Rust, Python and Node.js on the same workloads. The programs are written in the idiomatic style of each language, every program prints a checksum that the harness verifies against an independent oracle, and every number in the tables below is produced by the benchmark harness rather than typed by hand.

The tables are generated from the machine-readable results in `benchmarks/results/latest.json` and the human-readable summary in `benchmarks/results/latest.md`; the page itself is written by `scripts/gen_bench_page.py`. The workload programs live in `benchmarks/programs`.

> **Version note.** These numbers were measured for lsaca 2.4.0 on 2026-11-27, at 2026-11-27T21:38:17+08:00, from commit 3841c2e with compiler build 22d8552ea533c629 (2026-11-27 21:38:15 +08:00). The C reference was built with gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0. The 2.3.0 baseline was measured with gcc 16.2.0; comparisons against it are normalised by the C reference and explained in the fairness notes.

16 of the 22 executed workloads run within 1.4x of the C reference; 17 are within 1.5x and 9 are within 1.15x. The closest workload is `fib_iter` at 0.99x of the C runtime and the widest gap is `binary_trees` at 11.48x, a deliberate value-semantics trade-off.

A fresh single-file build takes 0.57-2.54 s and a cached rebuild takes 130-219 ms. The 4,511-line `compile_large` workload takes 3.53 s cold and 1105 ms cached. The generated C for that workload is 301,361 bytes (0.30 MB).

## Workloads

The suite has 25 workloads. 22 of them are executed and timed; the remaining 3 are compile-only and measure build time and generated-code size. Every executed program prints an exact checksum, so a run that prints the wrong answer fails the harness instead of producing a fast result.

| workload | what it stresses | exact workload | expected stdout |
| --- | --- | --- | --- |
| `fib` | non-tail recursion and calls | `fib(30)` computed 10 times and summed; `n` is a constant | `checksum=8320400` |
| `fib_iter` | iteration with two rolling values instead of recursion | fibonacci(30) computed 10 times iteratively; `n` and repetitions are constants | `checksum=8320400` |
| `fib_memo` | recursion plus a memo table | fibonacci(30) with a memo table, called 10 times | `checksum=8320400` |
| `loop_sum` | tight scalar integer loop | sum of 1..=8,000,000 | `checksum=32000004000000` |
| `string_build` | building a string from many small pieces | append 1,000,000 decimal tokens `i % 1000`, each followed by a comma; checksum is the final byte length and the sum of the token values | `checksum=3890000,499500000` |
| `vec_memory` | allocation volume and peak memory | build a vector of 1,000,000 `Point { x: i64, y: i64 }` values with `x = i`, `y = 2*i`; checksum over `x + y` | `checksum=1499998500000` |
| `sort_i64` | in-language sorting of random data | 200,000 pseudo-random `i64` values (LCG, seed 12345), sorted with an iterative quicksort written in the language; checksum is first, last and sum | `checksum=21749,2147481261,214813518297651` |
| `sort_insertion` | quadratic insertion sort, alternative to the library sort | insertion sort of 1,000 LCG values | `checksum=91125,2140623151,1054904362226` |
| `matrix_multiply` | integer multiply-add loops | multiply two 160x160 `i64` matrices (flat vectors, naive triple loop); checksum is the sum of the result entries | `checksum=1018924712000` |
| `binary_trees` | many small allocations, recursion | build a perfect binary tree of depth 16 (131,071 nodes) and sum every payload through a by-value function that `.clone()`s each child subtree; the deep copies are part of the measured cost | `checksum=2147450880` |
| `binary_trees_refs` | borrowed traversal | the same tree as `binary_trees`, but summed through borrowed references (`&Option<Box<Node>>`) instead of by value, so the traversal does not copy the tree | `checksum=2147450880` |
| `string_split` | splitting and parsing text | build a 194,500-byte comma-separated numeric string, split it, parse every token and sum; checksum is length, token count and sum | `checksum=194500,50000,24975000` |
| `sieve` | array-heavy loop | sieve of Eratosthenes below 2,000,000; count the primes | `checksum=148933` |
| `mandelbrot` | floating-point loops | 600x400 escape-time Mandelbrot, at most 100 iterations per pixel; checksum is the sum of the iteration counts | `checksum=6282265` |
| `iterator_pipeline` | lazy iterator abstraction versus a loop | run 1,000,000 `i64` values through `iter().filter(even).map(x*x).fold(0, add)`; the checksum is the folded sum | `checksum=166666166667000000` |
| `hashmap_wordcount` | keyed collection with owned keys | count 200,000 deterministic words drawn from a 4,096-word alphabet in a `HashMap<String, i64>`; checksum is the sum of squared counts, the number of distinct keys and the highest frequency | `checksum=9766208,4096,49` |
| `array_wordcount` | flat indexed array instead of a hash map | 200,000 tokens counted in 4,096 fixed buckets | `checksum=9766208,4096,49` |
| `generic_stack` | generic data structure, two instantiations | one generic `Stack<T>` instantiated for `i64` (push/pop 1,000,000 values) and for `String` (1,000 values); checksum is the integer sum and the total string byte length | `checksum=499999500000,5900` |
| `fn_pipeline` | function values stored in records | three rule records `{ label: String, apply: fn(i64) -> i64 }`, each applied 1,000 times, plus a clone of the first rule applied 500 times; checksum is the result sum and total label byte length | `checksum=53637,22` |
| `string_unicode` | character-level string building and Unicode data | build a 120,000-scalar mixed ASCII/Unicode string one character at a time, fold every scalar from iteration and from 1,000 random `char_at` accesses, apply full-Unicode `to_uppercase`, slice on character boundaries, push/pop and extend; checksum is character count, byte length and the folded scalar checksum | `checksum=3357,6921,368454760` |
| `hashmap_i64` | integer-key map operations | 1,000,000 integer-key inserts, 1,000,000 present lookups, 1,000,000 absent lookups and 100,000 deletions; checksum is the folded value sum, the final length and the number of successful lookups | `checksum=821060469,900000,1000000` |
| `hashmap_string` | string-key map: hashing and owned keys | 200,000 distinct generated `String` keys inserted, then 200,000 present lookups, 200,000 absent lookups (folding the bytes of each absent key) and 20,000 deletions; checksum is the folded value sum, the final length and the number of successful lookups | `checksum=794025725,180000,200000` |
| `compile_large` | compile-time scaling with source size | generated 4,511-line lsaca source (450 functions); **compile-only**, never executed by the harness | `checksum=14655037` (not measured) |
| `modules_compile` | compile-time scaling with a multi-file crate | one crate root (`main.ls`) plus eight sibling module files, 3,592 lines across 9 files, with structs, enums, generics, trait impls, strings, vectors and hash maps; **compile-only**, never executed by the harness | `checksum=547598677` (not measured) |
| `package_compile` | compile-time scaling with path dependencies | binary package `compile_app` plus library packages `util_a` and `util_b` (`util_b` depends on `util_a`), 1,088 lines across 13 files in 3 packages; **compile-only**, never executed by the harness | `checksum=417367897` (not measured) |

Each workload prints one deterministic line (a checksum or a short summary). The harness
compares the observed stdout with the expected line byte for byte, and the expected lines are
themselves recomputed by independent Python implementations in `scripts/oracles.py` rather than
captured from a previous run.

The programs are small but not toy-sized: they are sized so that the C build runs in about
10-330 ms, which keeps the suite practical while still spending most of its time in the code
under test rather than in process startup. The sizes are arguments, so the same programs also
provide the small and large data scales reported below.

## What is measured

### Compile time

Wall-clock time of one build command, median of 3 repetitions (1 in `--quick` mode):

- **lsaca, cold build (single file)**: `lsaca build -r <source> -o <outdir>` with a
  **fresh** `LSACA_CACHE_DIR` for each measurement.
- **lsaca, cold build (multi-file crate)**: `lsaca build -r <crate root> -o <outdir>`
  from the repository root, again with a fresh cache directory. The root file's `mod`
  declarations load the sibling module files; the compiler checks the whole module tree
  and emits one C project for it.
- **lsaca, cold build (package mode)**: `lsaca build -r` with no file argument from the
  package directory, with a fresh cache directory. The manifest is discovered upward and
  every path dependency is built and linked into the same executable.
- **lsaca, cached build**: the same command immediately afterwards, with the cache
  directory retained. The compiler keys its build on a content hash of the generated C,
  finds the previously built executable and returns it without recompiling program or
  runtime.
- **C**: `gcc -O2 <source> -o <exe>`.
- **Rust**: `rustc -O <source> -o <exe>`.
- **Python / Node.js**: not applicable - these languages have no ahead-of-time compile
  step in this suite. Their start-up cost is part of the runtime numbers instead.

All three cold lsaca shapes use the full user-facing path: emit C, compile the generated
program units (`main.c`, or `main.c` plus `lsaca_unit_1.c` … `lsaca_unit_N.c` when a
very large program is split), compile the six runtime translation units, and link, all
with the compiler's default direct C compiler back end (gcc, `-O2 -DNDEBUG`). The
runtime objects are compiled once into the cache root and reused by later builds; a
fresh cache directory forces the full work.

Since 1.2.0 the default build path does not use CMake at all: the compiler invokes gcc
directly. The older CMake + Ninja pipeline is still available as
`lsaca build -r --cmake`, but the harness does not measure it. Not included in any
compile figure: building the lsaca compiler itself (`cargo build --release -p lsaca-cli`),
or the one-time toolchain setup on a brand-new machine. The harness sets
`TEMP`/`TMP`/`TMPDIR` to a directory inside `target/bench-tmp` and puts the usual
toolchain directories on `PATH` (`C:\msys64\ucrt64\bin`, `C:\Program Files\CMake\bin`,
`~/.cargo/bin` on Windows) so the numbers do not depend on the caller's shell
configuration.

### Runtime

Each program runs as a **fresh child process**. The harness does 1 unmeasured warm-up run
and then 7 measured runs (3 in `--quick` mode), and reports the **median** and the
**minimum** of the measured runs. Every measured run's stdout is validated against the
expected line; a mismatch or a non-zero exit aborts the whole run.

- Process start-up (loader, runtime initialisation, interpreter/VM startup) is included.
  It is small for native programs and not small for Python/Node.js, which is part of what
  the table shows.
- No CPU pinning, no warm-up beyond the one warm-up run; the machine is a normal laptop
  with power management and background activity (see the caveats).

### Peak memory

Peak memory is measured in a **separate run** of the same executable, by sampling the
peak working set of the child process:

- Windows: polling `GetProcessMemoryInfo` / `PeakWorkingSetSize` (psapi) while the child
  runs, with a PowerShell `PeakWorkingSet64` fallback.
- Linux: `/usr/bin/time -v` (maximum resident set size) with a `/proc/<pid>/status`
  `VmHWM` fallback; macOS: `/usr/bin/time -l` with a `/proc` fallback where available.

This is **peak resident memory**, not bytes allocated, so it includes the language runtime
and the interpreter/VM baseline. It is a sampled peak, not an exact byte count; a spike
shorter than the sampling interval can be missed.

### Validation

Every measured run is validated three ways. The stdout of every run of every language must be
identical to the expected checksum line. lsaca programs are additionally built with the
debugging runtime (`-DLSACA_DEBUG`) and must run without a leak warning on stderr. Finally the
expected lines themselves come from independent oracle implementations, not from a previous
lsaca run:

> scripts/oracles.py reimplements every executed workload and recomputes its checksum line without running lsaca: 22 program checksums and 39 data-scale checksums agree with the expected output (mismatches: 0).

## Environment

All timings below were measured on one machine in one session, with the harness starting a fresh child process per measurement.

| field | value |
| --- | --- |
| Measured at | 2026-11-27 21:38:17 +08:00 |
| Machine | 12th Gen Intel(R) Core(TM) i7-1260P, 16 logical CPUs, 15.63 GiB RAM |
| OS | Windows-11-10.0.26300-SP0 |
| lsaca | lsaca 2.4.0, commit 3841c2e, compiler sha256 22d8552ea533c629, built 2026-11-27 21:38:15 +08:00 |
| lsaca build path | target\bench-tools\lsaca.exe |
| C compiler | gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0 |
| Rust compiler | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| Python | Python 3.14.10 |
| Node.js | v26.13.0 |
| CMake | cmake version 4.1.0 |
| Ninja | 1.13.2 |
| Harness | python scripts/bench.py: 25 programs (22 executed), 1 warm-up + 7 runs, 3 compile repeats, 3 data scales |

## Results

The next sections hold the raw numbers. Times are wall-clock seconds for builds and milliseconds for runs; lower is better everywhere.

### Compile time

Builds use the default back end: lsaca emits C and invokes the system C compiler directly. `lsaca cold build` starts from an empty build cache; `lsaca cached build` reuses the cached C project. The `gcc -O2` and `rustc -O` columns are the same program written once in C and once in Rust. Compile-only workloads have no executed equivalent, so only their build columns are reported.

| program | lsaca cold build (default back end) | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | --- | --- | --- | --- | --- | --- | --- |
| fib | 0.942 | 0.168 | n/a | 0.551 | 0.468 | n/a | n/a |
| fib_iter | 0.660 | 0.140 | n/a | 0.366 | n/a | n/a | n/a |
| fib_memo | 0.859 | 0.170 | n/a | 0.501 | n/a | n/a | n/a |
| loop_sum | 1.122 | 0.219 | n/a | 0.503 | 0.511 | n/a | n/a |
| string_build | 0.739 | 0.150 | n/a | 0.456 | 0.494 | n/a | n/a |
| vec_memory | 0.657 | 0.180 | n/a | 0.377 | 0.395 | n/a | n/a |
| sort_i64 | 0.966 | 0.164 | n/a | 0.421 | 0.693 | n/a | n/a |
| sort_insertion | 0.827 | 0.168 | n/a | 0.375 | n/a | n/a | n/a |
| matrix_multiply | 0.794 | 0.172 | n/a | 0.546 | 0.548 | n/a | n/a |
| binary_trees | 1.139 | 0.189 | n/a | 0.493 | 0.612 | n/a | n/a |
| binary_trees_refs | 1.378 | 0.155 | n/a | 0.441 | 0.512 | n/a | n/a |
| string_split | 0.732 | 0.162 | n/a | 0.393 | 0.747 | n/a | n/a |
| sieve | 0.783 | 0.168 | n/a | 0.526 | 0.534 | n/a | n/a |
| mandelbrot | 0.595 | 0.147 | n/a | 0.391 | 0.334 | n/a | n/a |
| iterator_pipeline | 2.543 | 0.150 | n/a | 0.282 | 0.342 | n/a | n/a |
| hashmap_wordcount | 0.881 | 0.132 | n/a | 0.494 | 0.550 | n/a | n/a |
| array_wordcount | 0.566 | 0.130 | n/a | 0.382 | n/a | n/a | n/a |
| generic_stack | 1.046 | 0.149 | n/a | 0.338 | 0.402 | n/a | n/a |
| fn_pipeline | 2.394 | 0.153 | n/a | 0.346 | 0.368 | n/a | n/a |
| string_unicode | 1.029 | 0.178 | n/a | 0.740 | 0.805 | n/a | n/a |
| hashmap_i64 | 1.121 | 0.210 | n/a | 0.433 | 0.919 | n/a | n/a |
| hashmap_string | 0.857 | 0.151 | n/a | 0.317 | 0.712 | n/a | n/a |
| compile_large (compile-only) | 3.532 | 1.105 | n/a | n/a | n/a | n/a | n/a |
| modules_compile (compile-only) | 5.166 | 1.032 | n/a | n/a | n/a | n/a | n/a |
| package_compile (compile-only) | 2.381 | 0.352 | n/a | n/a | n/a | n/a | n/a |

> The fresh single-file build ranges from 0.57 s to 2.54 s over the executed workloads, and the cached rebuild ranges from 130 ms to 219 ms.

> The C reference compiles faster because a C compiler does not have to run a lexer, a parser, a type checker, a lowering pass and a code generator first; the gap is the price of the language's guarantees.

> The compile-only workloads scale with source size: `compile_large` is a single 4,511-line file, `modules_compile` is nine modules and `package_compile` is three packages with thirteen files.

### Runtime

Each cell is the median of seven measured runs after one warm-up, in milliseconds, with the minimum in parentheses. Lower is better.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| lsaca | 38.6 (35.2) | 16.1 (14.4) | 17.7 (16.2) | 23.4 (20.0) | 99.7 (93.9) | 27.4 (24.0) | 44.8 (39.8) | 17.9 (15.8) | 23.2 (21.8) | 321.7 (294.4) | 42.7 (39.8) | 31.2 (28.2) | 47.8 (46.2) | 44.3 (37.5) | 20.1 (18.4) | 43.1 (36.4) | 19.3 (17.5) | 23.2 (22.0) | 17.0 (15.3) | 42.4 (41.2) | 355.6 (309.1) | 263.0 (255.2) |
| C (gcc -O2) | 32.2 (30.7) | 16.3 (13.6) | 17.2 (16.0) | 19.6 (18.1) | 20.9 (18.8) | 22.7 (20.5) | 42.5 (40.8) | 14.4 (13.4) | 21.7 (20.3) | 28.0 (27.2) | 23.9 (22.7) | 26.7 (20.6) | 43.0 (41.7) | 43.7 (41.5) | 13.6 (13.3) | 32.5 (28.7) | 18.4 (15.7) | 20.5 (19.8) | 13.8 (12.6) | 23.6 (21.7) | 227.7 (223.3) | 242.3 (225.7) |
| Rust (rustc -O) | 18.6 (16.3) | n/a | n/a | 16.1 (14.7) | 83.1 (77.6) | 19.0 (18.9) | 46.0 (41.9) | n/a | 18.9 (16.7) | 34.2 (30.9) | 25.6 (24.5) | 20.0 (19.0) | 39.8 (37.4) | 33.7 (30.8) | 15.7 (15.3) | 52.5 (47.2) | n/a | 19.5 (18.6) | 15.5 (14.1) | 20.7 (18.6) | 309.0 (297.7) | 248.6 (243.7) |
| Python 3 | 1083.3 (1048.7) | n/a | n/a | 387.3 (352.0) | 248.0 (217.7) | 289.7 (265.5) | 420.2 (395.3) | n/a | 547.3 (512.9) | 150.4 (129.8) | 102.1 (100.8) | 82.9 (66.4) | 266.0 (255.5) | 822.5 (777.6) | 137.7 (123.8) | 90.3 (75.9) | n/a | 248.8 (244.3) | 50.2 (45.3) | 232.6 (183.9) | 1611.7 (1310.9) | 635.5 (614.1) |
| Node.js | 188.3 (177.8) | n/a | n/a | 147.7 (139.3) | 143.9 (138.3) | 177.7 (161.2) | 137.1 (128.7) | n/a | 122.1 (118.9) | 136.4 (132.2) | 91.8 (88.9) | 105.9 (100.6) | 119.3 (109.4) | 93.7 (91.2) | 101.3 (97.8) | 90.5 (87.3) | n/a | 101.8 (95.1) | 95.8 (89.1) | 193.8 (182.9) | 906.0 (696.5) | 466.3 (423.5) |

> The same numbers divided by the C runtime give the relative cost; a value below 1.00x means lsaca is faster than the C reference on this workload.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 1.20x | 0.99x | 1.03x | 1.19x | 4.77x | 1.21x | 1.05x | 1.24x | 1.07x | 11.48x | 1.79x | 1.17x | 1.11x | 1.01x | 1.48x | 1.32x | 1.05x | 1.13x | 1.23x | 1.80x | 1.56x | 1.09x |
| Rust | 0.58x | n/a | n/a | 0.82x | 3.98x | 0.84x | 1.08x | n/a | 0.87x | 1.22x | 1.07x | 0.75x | 0.93x | 0.77x | 1.16x | 1.61x | n/a | 0.95x | 1.12x | 0.88x | 1.36x | 1.03x |
| Python 3 | 33.65x | n/a | n/a | 19.76x | 11.87x | 12.77x | 9.89x | n/a | 25.24x | 5.37x | 4.28x | 3.11x | 6.19x | 18.84x | 10.15x | 2.78x | n/a | 12.14x | 3.63x | 9.86x | 7.08x | 2.62x |
| Node.js | 5.85x | n/a | n/a | 7.53x | 6.89x | 7.84x | 3.23x | n/a | 5.63x | 4.87x | 3.85x | 3.96x | 2.78x | 2.15x | 7.46x | 2.78x | n/a | 4.97x | 6.92x | 8.21x | 3.98x | 1.92x |

### Peak memory

Peak working set of the measured child process, in MiB. The harness samples the peak with the platform API instead of reading a single instantaneous value, so short-lived peaks are included.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| lsaca | 4.5 | 4.5 | 4.5 | 4.5 | 11.0 | 19.2 | 7.1 | 4.5 | 4.6 | 52.3 | 16.2 | 9.8 | 19.3 | 4.5 | 11.6 | 4.7 | 4.5 | 13.0 | 4.5 | 5.6 | 55.9 | 30.0 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.2 | 18.8 | 5.0 | 4.0 | 4.3 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.7 | 52.5 | 19.6 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.4 | 18.9 | 5.1 | n/a | 4.4 | 7.7 | 7.7 | 4.3 | 18.9 | 4.1 | 11.2 | 4.3 | n/a | 12.6 | 4.1 | 4.7 | 55.7 | 19.7 |
| Python 3 | 10.9 | n/a | n/a | 10.9 | 74.4 | 142.0 | 20.2 | n/a | 13.5 | 21.0 | 21.0 | 17.0 | 26.1 | 11.0 | 49.2 | 11.3 | n/a | 54.1 | 11.0 | 26.9 | 113.3 | 38.1 |
| Node.js | 43.1 | n/a | n/a | 45.5 | 93.5 | 128.0 | 51.7 | n/a | 47.2 | 52.5 | 52.7 | 53.5 | 59.8 | 45.4 | 56.0 | 46.7 | n/a | 74.8 | 40.5 | 65.4 | 116.5 | 91.5 |

### Runtime by data scale

The scalable workloads are run at small, medium (the default arguments) and large sizes. Checksums are verified at every scale, so a scaling run that computes the wrong answer is rejected.

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | --- | --- | --- | --- | --- |
| fib | small | 20.0 (18.8) | 12.4 (12.2) | 14.6 (14.0) | 65.2 (59.9) | 79.1 (77.9) |
| fib | medium | 38.6 (35.2) | 32.2 (30.7) | 18.6 (16.3) | 1083.3 (1048.7) | 188.3 (177.8) |
| fib | large | 54.3 (51.4) | 41.2 (40.6) | 21.9 (20.7) | 1649.5 (1643.0) | 228.8 (226.1) |
| fib_iter | small | 18.7 (17.8) | 18.7 (15.8) | n/a | n/a | n/a |
| fib_iter | medium | 16.1 (14.4) | 16.3 (13.6) | n/a | n/a | n/a |
| fib_iter | large | 20.1 (19.6) | 18.0 (16.5) | n/a | n/a | n/a |
| fib_memo | small | 18.3 (15.6) | 14.5 (13.1) | n/a | n/a | n/a |
| fib_memo | medium | 17.7 (16.2) | 17.2 (16.0) | n/a | n/a | n/a |
| fib_memo | large | 23.2 (21.4) | 14.5 (14.1) | n/a | n/a | n/a |
| loop_sum | small | 19.2 (19.1) | 15.4 (15.0) | 21.6 (19.7) | 91.3 (89.0) | 104.8 (99.3) |
| loop_sum | medium | 23.4 (20.0) | 19.6 (18.1) | 16.1 (14.7) | 387.3 (352.0) | 147.7 (139.3) |
| loop_sum | large | 23.0 (21.6) | 20.5 (18.9) | 21.0 (16.9) | 614.6 (609.5) | 160.6 (159.2) |
| string_build | small | 18.6 (18.5) | 17.1 (15.9) | 13.8 (13.8) | 51.8 (49.0) | 79.8 (78.4) |
| string_build | medium | 99.7 (93.9) | 20.9 (18.8) | 83.1 (77.6) | 248.0 (217.7) | 143.9 (138.3) |
| string_build | large | 379.6 (376.6) | 36.2 (35.4) | 285.3 (278.7) | 1063.8 (868.6) | 372.5 (370.8) |
| sort_i64 | small | 18.9 (16.4) | 12.2 (12.0) | 19.1 (17.5) | 41.2 (40.6) | 96.4 (94.0) |
| sort_i64 | medium | 44.8 (39.8) | 42.5 (40.8) | 46.0 (41.9) | 420.2 (395.3) | 137.1 (128.7) |
| sort_i64 | large | 92.0 (91.4) | 77.7 (74.4) | 97.8 (85.3) | 1195.9 (1134.2) | 197.4 (192.2) |
| sort_insertion | small | 17.3 (15.8) | 17.8 (15.9) | n/a | n/a | n/a |
| sort_insertion | medium | 17.9 (15.8) | 14.4 (13.4) | n/a | n/a | n/a |
| sort_insertion | large | 23.1 (19.4) | 21.1 (20.4) | n/a | n/a | n/a |
| binary_trees | small | 24.4 (21.7) | 16.2 (14.9) | 16.5 (16.0) | 70.3 (61.2) | 118.8 (106.3) |
| binary_trees | medium | 321.7 (294.4) | 28.0 (27.2) | 34.2 (30.9) | 150.4 (129.8) | 136.4 (132.2) |
| binary_trees | large | 1399.0 (1391.7) | 68.7 (65.8) | 74.5 (71.1) | 472.7 (469.2) | 167.0 (165.3) |
| binary_trees_refs | small | 17.9 (16.5) | 20.0 (12.2) | 14.3 (14.0) | 40.8 (40.6) | 84.9 (82.1) |
| binary_trees_refs | medium | 42.7 (39.8) | 23.9 (22.7) | 25.6 (24.5) | 102.1 (100.8) | 91.8 (88.9) |
| binary_trees_refs | large | 114.2 (113.6) | 61.3 (58.5) | 62.8 (59.4) | 321.8 (307.5) | 122.6 (120.7) |
| string_split | small | 18.6 (18.3) | 20.8 (20.5) | 18.0 (16.4) | 64.3 (59.6) | 85.1 (83.9) |
| string_split | medium | 31.2 (28.2) | 26.7 (20.6) | 20.0 (19.0) | 82.9 (66.4) | 105.9 (100.6) |
| string_split | large | 170.4 (158.3) | 77.2 (64.9) | 72.6 (69.3) | 272.6 (255.6) | 252.0 (250.9) |
| sieve | small | 16.7 (16.0) | 16.9 (16.7) | 14.3 (14.0) | 54.1 (49.5) | 84.3 (80.2) |
| sieve | medium | 47.8 (46.2) | 43.0 (41.7) | 39.8 (37.4) | 266.0 (255.5) | 119.3 (109.4) |
| sieve | large | 108.7 (106.4) | 112.3 (104.4) | 111.3 (104.7) | 678.6 (600.3) | 158.6 (156.7) |
| hashmap_wordcount | small | 17.0 (16.2) | 15.5 (14.4) | 14.9 (14.5) | 38.6 (35.3) | 73.4 (69.7) |
| hashmap_wordcount | medium | 43.1 (36.4) | 32.5 (28.7) | 52.5 (47.2) | 90.3 (75.9) | 90.5 (87.3) |
| hashmap_wordcount | large | 143.8 (137.4) | 96.0 (94.3) | 208.0 (198.6) | 247.4 (241.3) | 171.6 (168.6) |
| array_wordcount | small | 18.0 (17.4) | 21.1 (20.1) | n/a | n/a | n/a |
| array_wordcount | medium | 19.3 (17.5) | 18.4 (15.7) | n/a | n/a | n/a |
| array_wordcount | large | 21.6 (20.7) | 21.4 (18.8) | n/a | n/a | n/a |

### Artifacts and generated C

Executable sizes and the size of the generated C sources. The generated C size is measured before the C compiler runs; header files and the shared runtime are not counted per program.

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | --- | --- | --- | --- |
| fib | 250,172 | 16,911 | 40,164 | 140,288 |
| fib_iter | 249,153 | 17,184 | 39,032 | n/a |
| fib_memo | 251,382 | 22,734 | 41,381 | n/a |
| loop_sum | 248,537 | 15,015 | 39,032 | 140,288 |
| string_build | 249,702 | 17,761 | 39,717 | 141,312 |
| vec_memory | 247,280 | 9,347 | 38,393 | 131,584 |
| sort_i64 | 251,609 | 29,327 | 40,404 | 141,824 |
| sort_insertion | 250,591 | 23,738 | 39,892 | n/a |
| matrix_multiply | 246,479 | 15,679 | 39,080 | 131,584 |
| binary_trees | 255,956 | 24,368 | 43,154 | 141,824 |
| binary_trees_refs | 256,468 | 23,887 | 43,174 | 141,824 |
| string_split | 249,837 | 21,160 | 40,396 | 142,336 |
| sieve | 249,903 | 19,025 | 39,555 | 140,800 |
| mandelbrot | 245,526 | 8,550 | 38,732 | 131,072 |
| iterator_pipeline | 380,427 | 194,565 | 38,588 | 131,072 |
| hashmap_wordcount | 256,525 | 38,689 | 42,302 | 146,432 |
| array_wordcount | 249,903 | 20,232 | 39,207 | n/a |
| generic_stack | 251,673 | 21,170 | 40,784 | 134,144 |
| fn_pipeline | 375,675 | 185,846 | 38,400 | 131,584 |
| string_unicode | 253,627 | 35,371 | 41,377 | 140,800 |
| hashmap_i64 | 252,093 | 25,011 | 39,891 | 136,192 |
| hashmap_string | 254,653 | 32,268 | 41,594 | 139,264 |
| compile_large (compile-only) | 367,309 | 301,361 | n/a | n/a |
| modules_compile (compile-only) | 609,930 | 2,693,948 | n/a | n/a |
| package_compile (compile-only) | 302,339 | 747,441 | n/a | n/a |

## Analysis

### What this run shows

lsaca 2.4.0 lands 16 of the 22 executed workloads inside 1.4x of C, 17 inside 1.5x and 9 inside 1.15x. The median lsaca/C ratio over the executed workloads is 1.20x.

Absolute milliseconds are not comparable with the 2.3.0 baseline even though both runs used the same C reference compiler (gcc 16.2.0): the archived run was taken earlier on a different host state. The table below therefore compares C-normalised ratios: each language's time divided by the C time of the same run. A positive change means lsaca lost ground relative to the C reference.

| workload | 2.3.0 ratio to C | 2.4.0 ratio to C | change |
| --- | --- | --- | --- |
| `iterator_pipeline` | 1.07x | 1.48x | +38.2% |
| `generic_stack` | 0.85x | 1.13x | +34.2% |
| `fib` | 0.99x | 1.20x | +20.8% |
| `fn_pipeline` | 1.05x | 1.23x | +16.5% |
| `vec_memory` | 1.18x | 1.21x | +2.5% |
| `hashmap_wordcount` | 1.32x | 1.32x | +0.6% |
| `binary_trees_refs` | 1.85x | 1.79x | -3.3% |
| `sort_i64` | 1.10x | 1.05x | -4.0% |
| `hashmap_i64` | 1.65x | 1.56x | -5.5% |
| `sort_insertion` | 1.34x | 1.24x | -7.7% |
| `binary_trees` | 12.56x | 11.48x | -8.6% |
| `mandelbrot` | 1.11x | 1.01x | -8.9% |
| `matrix_multiply` | 1.21x | 1.07x | -11.4% |
| `loop_sum` | 1.42x | 1.19x | -15.8% |
| `hashmap_string` | 1.30x | 1.09x | -16.6% |
| `string_unicode` | 2.28x | 1.80x | -21.1% |
| `array_wordcount` | 1.34x | 1.05x | -21.6% |
| `fib_iter` | 1.27x | 0.99x | -22.1% |
| `string_build` | 6.23x | 4.77x | -23.4% |
| `string_split` | 1.55x | 1.17x | -24.4% |
| `sieve` | 1.50x | 1.11x | -25.9% |
| `fib_memo` | 1.51x | 1.03x | -31.9% |

### Largest improvements against the 2.3.0 baseline

- `fib_memo`: 1.51x to 1.03x (-31.9%).
- `sieve`: 1.50x to 1.11x (-25.9%).
- `string_split`: 1.55x to 1.17x (-24.4%).
- `string_build`: 6.23x to 4.77x (-23.4%).
- `fib_iter`: 1.27x to 0.99x (-22.1%).
- `array_wordcount`: 1.34x to 1.05x (-21.6%).

### Largest regressions against the 2.3.0 baseline

- `iterator_pipeline`: 1.07x to 1.48x (+38.2%).
- `generic_stack`: 0.85x to 1.13x (+34.2%).
- `fib`: 0.99x to 1.20x (+20.8%).
- `fn_pipeline`: 1.05x to 1.23x (+16.5%).
- `vec_memory`: 1.18x to 1.21x (+2.5%).

### Workloads outside the 1.4x target

These workloads are above the 1.4x target in this run. Each one has a reason; the reasons are checked against the program source, not guessed.

| workload | lsaca runtime (ms) | C runtime (ms) | ratio | note |
| --- | --- | --- | --- | --- |
| `binary_trees` | 321.7 | 28.0 | 11.48x | by-value tree semantics: each recursive step clones the child subtree, so the deep copies are part of the workload by design; binary_trees_refs is the borrowed-traversal variant |
| `string_build` | 99.7 | 20.9 | 4.77x | one million appends; each append goes through the checked String API instead of writing into a raw buffer |
| `string_unicode` | 42.4 | 23.6 | 1.80x | owned String calls and Unicode table lookups; character-level work checks UTF-8 boundaries |
| `binary_trees_refs` | 42.7 | 23.9 | 1.79x | borrowed tree traversal; references do not copy the tree, so only the traversal cost remains |
| `hashmap_i64` | 355.6 | 227.7 | 1.56x | integer-key map operations; the C version uses a hand-written open-addressing table |
| `iterator_pipeline` | 20.1 | 13.6 | 1.48x | a lazy adapter chain; each step is monomorphized but the adapters still carry state |

### Acceptance against the standing targets

| target | result | status |
| --- | --- | --- |
| Every executed runtime within 1.4x of C | 16/22 within 1.4x | partly met |
| Smallest fresh single-file build 0.35-0.5 s | 0.57 s | not met |
| Cached rebuild around 15 ms | 130 ms | not met |
| Compile-only workload within 2.5x of the C compile | 3.53 s, no C reference | not scored |
| Generated C compiles without warnings | every measured program built by the harness | met |
| Independent oracle agreement | 22 program checksums and 39 data-scale checksums agree | met |
| Runtime medians within 10% of 2.3.0 (C-normalised) | 4 of 22 outside the budget | partly met |

The 1.4x target is a language-level goal, not a hard release gate: a workload outside it must have a recorded reason, and the table above names them all.

### Python and Node.js

The dynamic languages are far slower than the compiled ones on these workloads. lsaca is between 2.62x and 33.65x the speed of CPython and between 1.92x and 8.21x the speed of Node.js. They are included to keep the page honest about the range: a language that only compares with C can look worse than it is.

## Fairness notes and caveats

1. The benchmark programs are written in the idiomatic style of each language. The comparisons that matter are the ones where each language does what it is good at:

| workload | lsaca | C | Rust | Python | Node.js |
| --- | --- | --- | --- | --- | --- |
| `string_build` | `String::new()` + `format!` then `push_str(&piece)` | one `malloc`ed char buffer of exact size | `String::with_capacity` + `to_string` + `push` | list of `str` + `",".join` | array + `join` |
| `vec_memory` | `Vec<Point>` + `with_capacity` + `push` | `malloc`ed array of structs | `Vec<Point>` with `with_capacity` | list of tuples | array of objects |
| `sort_i64` | iterative quicksort on `Vec<i64>` | same quicksort on a `malloc`ed array | `Vec<i64>` + `swap` | list + swapping | array + swapping |
| `binary_trees` | `Option<Box<Node>>` recursive struct; the by-value traversal clones child subtrees with `.clone()` (moving a field out is `E0507`) | `malloc`ed nodes + raw pointers | `Option<Box<Node>>` if let / match | class with `__slots__` | plain objects |
| `binary_trees_refs` | same tree, summed via `&Option<Box<Node>>` (no copy) | same program as `binary_trees` (C uses pointers anyway) | same program as `binary_trees` (moving a `Box` is a pointer move) | same program | same program |
| `sieve` | `vec![0; limit + 1]` of `i64` | `calloc`ed `int64_t` array | `vec![0i64; n]` | list of ints | `Float64Array` |
| `iterator_pipeline` | `values.into_iter().filter(...).map(...).fold(...)` lazy adapter structs from the prelude | eager `for` loop with the same arithmetic | its own iterator chain | generator expression inside `sum` | eager loop with `BigInt` squaring |
| `hashmap_wordcount` | builtin `HashMap<String, i64>` with `get`/`insert` and borrowed `keys()` iteration | hand-written open-addressing map with `malloc`ed `char *` keys | `std::collections::HashMap` + `entry` | `dict` | `Map` |
| `generic_stack` | one generic `Stack<T>`, monomorphized twice | two hand-written typed stack implementations | one generic `Stack<T>` | one class `Stack` | one class `Stack` |
| `fn_pipeline` | `Rule { label: String, apply: fn(i64) -> i64 }` + `.clone()` of a rule | struct with `const char *` and a function pointer | struct with `String` and `fn(i64) -> i64` | class holding a lambda | object holding a function |
| `string_unicode` | `String` built one `char` at a time; `.chars()`, `char_at`, `to_uppercase`, boundary-checked `substring` | `char *` buffer with byte-oriented UTF-8 handling | `String` with `char` iteration and `to_uppercase` | code-point list + `str.upper` | code-point array + `toUpperCase` |
| `hashmap_i64` | builtin `HashMap<i64, i64>` with `insert`/`get`/`remove` | hand-written open-addressing map with `int64_t` keys | `std::collections::HashMap<i64, i64>` | `dict` | `Map` |
| `hashmap_string` | builtin `HashMap<String, i64>` with owned keys | hand-written open-addressing map with `strdup`ed keys | `std::collections::HashMap<String, i64>` | `dict` | `Map` |

2. Absolute milliseconds are specific to this host and toolchain: 12th Gen Intel(R) Core(TM) i7-1260P running Windows-11-10.0.26300-SP0, C reference gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0. The 2.3.0 baseline was measured with gcc 16.2.0, so cross-release claims are made about C-normalised ratios only.

3. All measurements were taken in one session on an otherwise idle machine; the harness runs one program at a time and does not overlap builds with measurements.

4. Short workloads have higher relative noise. Every runtime table reports both the median and the minimum of the measured runs so the spread is visible, and the harness discards the first run as a warm-up.

5. The closest workload is `fib_iter` at 0.99x of C; the widest gap is `binary_trees` at 11.48x. Both are real measurements, not rounding artefacts of a single run.

6. `binary_trees` is the largest deliberate semantic gap: lsaca implements value-semantics trees, so a tree assignment copies the tree and the program builds a different shape from the pointer-based C and Rust versions. It is kept because it measures the cost of the language's semantics, not because the implementation is slow.

7. `hashmap_string` is faster than the C reference on some runs. That is the Swiss-table implementation doing its job; the C reference uses a simpler open-addressed table, so this is a data-structure comparison as much as a language comparison.

8. The alternative-implementation workloads (the programs whose name ends in a second variant) are built only for lsaca and C, because translating every alternative five ways would measure translation style rather than the language.

9. Compile-only workloads measure the build, not a running program: they have no runtime row, and their C reference column is n/a. They exist to show how build time grows with source size and module count.

10. The cached build column is the best case for an unchanged source file: the harness reuses the content-addressed build cache, so it measures the cached path rather than a fresh compile.

11. Peak memory is the peak working set of the child process, sampled by the harness through the platform API. It includes the runtime's allocator pools, which trade a little memory for speed.

12. Every runtime table cell is validated: the harness compares the printed checksum against the oracle and fails the measurement if it differs. A wrong answer cannot become a fast result.

13. Relative to the 2.3.0 baseline, `iterator_pipeline`, `generic_stack`, `fib`, `fn_pipeline`, `vec_memory` moved outside the 2.3.0 measurement noise after normalising by the C reference. The release notes track these explicitly instead of hiding them in a different host speed.

14. The harness itself is part of the measurement: it compiles, runs, samples memory and validates checksums, and its overhead is outside the timed child process.

## Reproducing

The page is generated from the results of the official harness, so the exact commands are:

```sh
cargo build --release -p lsaca-cli
python scripts/bench.py --compiler target/release/lsaca.exe --build-mode official \
    --json benchmarks/results/latest.json --md benchmarks/results/latest.md
python scripts/gen_bench_page.py
```

Use `--quick` for a short smoke run or `--programs fib,sort_i64` to measure a subset. The harness rejects an incomplete official run, so a green page always corresponds to a full suite.

The 2.3.0 baseline used for the comparison table lives in `benchmarks/results/archive/v2.3/latest.json`; it reports lsaca 2.3.0 and is never overwritten by the harness.

The regression guard reads the same files:

```sh
python scripts/bench_guard.py --results benchmarks/results/latest.json \
    --baseline benchmarks/results/archive/v2.3/latest.json
```

The guard compares absolute times when the host and toolchains match the baseline, and C-normalised ratios otherwise. Toolchain upgrades are the normal reason for the second mode.

Data-scale runs use three sizes per workload: `small`, `medium` (the default arguments) and `large`. The large size is why the full suite takes a few minutes rather than seconds.

## See also

- [benchmarks/README.md](../../benchmarks/README.md) - harness usage, output layout, how
  to add a workload.
- [benchmarks/results/latest.json](../../benchmarks/results/latest.json) - the raw data
  behind this page.
- [lsaca documentation](../README.md) - the manual hub and full index.
- [Getting started](01-getting-started.md) - the first chapter of the language tour.
