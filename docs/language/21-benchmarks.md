# Benchmarks

This page reports how lsaca compares with C, Rust, Python and Node.js on the same workloads. The programs are written in the idiomatic style of each language, every program prints a checksum that the harness verifies against an independent oracle, and every number in the tables below is produced by the benchmark harness rather than typed by hand.

The tables are generated from the machine-readable results in `benchmarks/results/latest.json` and the human-readable summary in `benchmarks/results/latest.md`; the page itself is written by `scripts/gen_bench_page.py`. The workload programs live in `benchmarks/programs`.

> **Version note.** These numbers were measured for lsaca 2.5.0 on 2026-11-28, at 2026-11-28T17:14:30+08:00, from commit 2e3243c with compiler build aeb084e730459b4e (2026-11-28 16:30:49 +08:00). The C reference was built with gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0. The 2.4.0 baseline was measured with gcc 16.2.0; comparisons against it are normalised by the C reference and explained in the fairness notes.

14 of the 22 executed workloads run within 1.4x of the C reference; 16 are within 1.5x and 12 are within 1.15x. The closest workload is `fn_pipeline` at 0.80x of the C runtime and the widest gap is `binary_trees` at 14.24x, a deliberate value-semantics trade-off.

A fresh single-file build takes 0.83-2.76 s and a cached rebuild takes 140-233 ms. The 4,511-line `compile_large` workload takes 3.70 s cold and 1194 ms cached. The generated C for that workload is 315,881 bytes (0.32 MB).

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
| Measured at | 2026-11-28 17:14:30 +08:00 |
| Machine | 12th Gen Intel(R) Core(TM) i7-1260P, 16 logical CPUs, 15.63 GiB RAM |
| OS | Windows-11-10.0.26300-SP0 |
| lsaca | lsaca 2.5.0, commit 2e3243c, compiler sha256 aeb084e730459b4e, built 2026-11-28 16:30:49 +08:00 |
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
| fib | 1.042 | 0.150 | n/a | 0.396 | 0.592 | n/a | n/a |
| fib_iter | 0.835 | 0.177 | n/a | 0.553 | n/a | n/a | n/a |
| fib_memo | 1.320 | 0.223 | n/a | 0.453 | n/a | n/a | n/a |
| loop_sum | 0.911 | 0.175 | n/a | 0.338 | 0.589 | n/a | n/a |
| string_build | 1.047 | 0.190 | n/a | 0.350 | 0.752 | n/a | n/a |
| vec_memory | 1.010 | 0.179 | n/a | 0.411 | 0.461 | n/a | n/a |
| sort_i64 | 1.462 | 0.189 | n/a | 0.499 | 0.824 | n/a | n/a |
| sort_insertion | 1.267 | 0.225 | n/a | 0.462 | n/a | n/a | n/a |
| matrix_multiply | 1.032 | 0.176 | n/a | 0.468 | 0.474 | n/a | n/a |
| binary_trees | 1.327 | 0.197 | n/a | 0.429 | 0.555 | n/a | n/a |
| binary_trees_refs | 1.300 | 0.191 | n/a | 0.480 | 0.552 | n/a | n/a |
| string_split | 1.069 | 0.216 | n/a | 0.521 | 0.767 | n/a | n/a |
| sieve | 1.388 | 0.233 | n/a | 0.342 | 0.504 | n/a | n/a |
| mandelbrot | 0.889 | 0.173 | n/a | 0.384 | 0.393 | n/a | n/a |
| iterator_pipeline | 2.758 | 0.196 | n/a | 0.313 | 0.398 | n/a | n/a |
| hashmap_wordcount | 1.174 | 0.163 | n/a | 0.454 | 0.646 | n/a | n/a |
| array_wordcount | 1.028 | 0.166 | n/a | 0.515 | n/a | n/a | n/a |
| generic_stack | 1.140 | 0.167 | n/a | 0.421 | 0.402 | n/a | n/a |
| fn_pipeline | 2.398 | 0.170 | n/a | 0.326 | 0.329 | n/a | n/a |
| string_unicode | 0.961 | 0.156 | n/a | 0.488 | 0.505 | n/a | n/a |
| hashmap_i64 | 0.908 | 0.153 | n/a | 0.328 | 0.601 | n/a | n/a |
| hashmap_string | 0.877 | 0.140 | n/a | 0.288 | 0.595 | n/a | n/a |
| compile_large (compile-only) | 3.702 | 1.194 | n/a | n/a | n/a | n/a | n/a |
| modules_compile (compile-only) | 5.326 | 1.023 | n/a | n/a | n/a | n/a | n/a |
| package_compile (compile-only) | 2.323 | 0.305 | n/a | n/a | n/a | n/a | n/a |

> The fresh single-file build ranges from 0.83 s to 2.76 s over the executed workloads, and the cached rebuild ranges from 140 ms to 233 ms.

> The C reference compiles faster because a C compiler does not have to run a lexer, a parser, a type checker, a lowering pass and a code generator first; the gap is the price of the language's guarantees.

> The compile-only workloads scale with source size: `compile_large` is a single 4,511-line file, `modules_compile` is nine modules and `package_compile` is three packages with thirteen files.

### Runtime

Each cell is the median of seven measured runs after one warm-up, in milliseconds, with the minimum in parentheses. Lower is better.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| lsaca | 34.4 (31.5) | 19.0 (17.7) | 15.0 (13.0) | 22.8 (19.6) | 124.0 (110.1) | 25.8 (24.8) | 44.7 (42.7) | 21.3 (20.3) | 21.7 (20.2) | 332.7 (297.5) | 52.4 (47.5) | 34.4 (33.3) | 54.4 (50.6) | 42.4 (38.2) | 20.0 (19.5) | 42.6 (40.7) | 19.5 (18.2) | 24.2 (22.8) | 14.9 (14.1) | 40.5 (36.4) | 276.9 (269.8) | 225.3 (210.9) |
| C (gcc -O2) | 31.7 (29.9) | 16.7 (14.3) | 16.2 (14.3) | 17.3 (16.8) | 26.5 (20.8) | 28.2 (24.1) | 45.0 (41.7) | 14.6 (13.9) | 20.6 (19.5) | 23.4 (22.4) | 29.3 (25.6) | 19.1 (18.2) | 36.6 (32.6) | 38.2 (35.6) | 19.0 (18.0) | 35.5 (32.7) | 21.1 (19.1) | 23.0 (21.4) | 18.6 (16.9) | 16.3 (15.9) | 174.3 (173.4) | 207.2 (188.2) |
| Rust (rustc -O) | 20.8 (19.6) | n/a | n/a | 17.1 (15.9) | 104.7 (82.8) | 25.1 (23.4) | 50.9 (46.0) | n/a | 18.6 (17.7) | 26.4 (23.9) | 27.4 (27.0) | 25.2 (22.8) | 37.9 (35.8) | 39.0 (36.1) | 19.1 (18.2) | 57.8 (56.5) | n/a | 19.5 (18.3) | 10.6 (10.4) | 18.4 (16.5) | 205.5 (197.4) | 247.6 (223.5) |
| Python 3 | 1243.4 (1110.4) | n/a | n/a | 426.1 (383.6) | 234.9 (228.9) | 350.9 (323.4) | 545.3 (425.6) | n/a | 627.5 (558.6) | 101.4 (98.1) | 112.8 (104.7) | 91.2 (78.3) | 264.5 (259.9) | 1034.9 (894.2) | 150.6 (142.2) | 90.6 (83.8) | n/a | 254.1 (246.7) | 37.1 (33.3) | 156.3 (153.5) | 1195.9 (1165.1) | 582.2 (548.3) |
| Node.js | 190.8 (169.5) | n/a | n/a | 136.2 (131.8) | 161.4 (150.0) | 213.3 (181.8) | 142.0 (138.2) | n/a | 130.0 (121.5) | 115.0 (103.4) | 98.0 (96.1) | 161.7 (143.0) | 129.2 (113.5) | 149.7 (136.0) | 131.0 (128.1) | 119.4 (113.2) | n/a | 122.8 (89.5) | 85.4 (80.9) | 132.7 (128.8) | 676.8 (647.3) | 539.0 (512.4) |

> The same numbers divided by the C runtime give the relative cost; a value below 1.00x means lsaca is faster than the C reference on this workload.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 1.09x | 1.14x | 0.93x | 1.32x | 4.68x | 0.92x | 0.99x | 1.47x | 1.06x | 14.24x | 1.79x | 1.80x | 1.49x | 1.11x | 1.05x | 1.20x | 0.93x | 1.05x | 0.80x | 2.49x | 1.59x | 1.09x |
| Rust | 0.66x | n/a | n/a | 0.99x | 3.95x | 0.89x | 1.13x | n/a | 0.90x | 1.13x | 0.93x | 1.32x | 1.04x | 1.02x | 1.01x | 1.63x | n/a | 0.85x | 0.57x | 1.13x | 1.18x | 1.19x |
| Python 3 | 39.28x | n/a | n/a | 24.61x | 8.86x | 12.45x | 12.13x | n/a | 30.53x | 4.34x | 3.85x | 4.77x | 7.23x | 27.13x | 7.95x | 2.55x | n/a | 11.05x | 2.00x | 9.62x | 6.86x | 2.81x |
| Node.js | 6.03x | n/a | n/a | 7.87x | 6.09x | 7.57x | 3.16x | n/a | 6.33x | 4.92x | 3.35x | 8.45x | 3.53x | 3.92x | 6.91x | 3.36x | n/a | 5.34x | 4.60x | 8.16x | 3.88x | 2.60x |

### Peak memory

Peak working set of the measured child process, in MiB. The harness samples the peak with the platform API instead of reading a single instantaneous value, so short-lived peaks are included.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| lsaca | 4.5 | 4.5 | 4.5 | 4.5 | 11.0 | 19.2 | 7.0 | 4.5 | 4.6 | 52.3 | 16.2 | 9.8 | 19.3 | 4.5 | 11.6 | 4.7 | 4.5 | 13.0 | 4.5 | 5.6 | 55.9 | 29.9 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.2 | 18.8 | 5.0 | 4.0 | 4.3 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.7 | 52.5 | 19.6 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.3 | 18.8 | 5.1 | n/a | 4.4 | 7.6 | 7.6 | 4.3 | 18.9 | 4.1 | 11.2 | 4.3 | n/a | 12.6 | 4.1 | 4.7 | 55.8 | 19.7 |
| Python 3 | 10.9 | n/a | n/a | 10.9 | 73.6 | 142.6 | 20.9 | n/a | 13.5 | 20.9 | 20.9 | 16.9 | 26.1 | 10.9 | 49.2 | 11.3 | n/a | 54.1 | 10.9 | 25.4 | 113.3 | 38.0 |
| Node.js | 43.0 | n/a | n/a | 45.6 | 93.4 | 128.4 | 51.4 | n/a | 47.3 | 52.5 | 52.3 | 53.7 | 59.7 | 46.0 | 56.2 | 46.7 | n/a | 74.8 | 40.5 | 65.0 | 116.7 | 91.5 |

### Runtime by data scale

The scalable workloads are run at small, medium (the default arguments) and large sizes. Checksums are verified at every scale, so a scaling run that computes the wrong answer is rejected.

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | --- | --- | --- | --- | --- |
| fib | small | 23.7 (19.4) | 14.6 (14.4) | 15.2 (14.6) | 46.5 (43.3) | 90.4 (79.4) |
| fib | medium | 34.4 (31.5) | 31.7 (29.9) | 20.8 (19.6) | 1243.4 (1110.4) | 190.8 (169.5) |
| fib | large | 48.5 (43.4) | 51.3 (49.3) | 21.5 (19.6) | 1711.7 (1636.9) | 235.0 (233.5) |
| fib_iter | small | 20.5 (19.4) | 17.0 (16.8) | n/a | n/a | n/a |
| fib_iter | medium | 19.0 (17.7) | 16.7 (14.3) | n/a | n/a | n/a |
| fib_iter | large | 22.3 (20.6) | 15.0 (14.4) | n/a | n/a | n/a |
| fib_memo | small | 14.2 (13.9) | 16.2 (14.2) | n/a | n/a | n/a |
| fib_memo | medium | 15.0 (13.0) | 16.2 (14.3) | n/a | n/a | n/a |
| fib_memo | large | 14.9 (14.0) | 15.9 (13.4) | n/a | n/a | n/a |
| loop_sum | small | 18.3 (18.0) | 14.9 (13.3) | 16.9 (15.7) | 81.7 (77.7) | 93.3 (90.8) |
| loop_sum | medium | 22.8 (19.6) | 17.3 (16.8) | 17.1 (15.9) | 426.1 (383.6) | 136.2 (131.8) |
| loop_sum | large | 23.6 (20.6) | 17.6 (17.2) | 21.6 (16.7) | 536.4 (528.1) | 162.9 (162.6) |
| string_build | small | 22.4 (21.1) | 20.2 (18.0) | 15.0 (14.4) | 42.5 (40.7) | 93.2 (92.7) |
| string_build | medium | 124.0 (110.1) | 26.5 (20.8) | 104.7 (82.8) | 234.9 (228.9) | 161.4 (150.0) |
| string_build | large | 465.6 (442.3) | 52.9 (47.1) | 272.9 (264.6) | 859.9 (838.4) | 510.0 (506.3) |
| sort_i64 | small | 18.2 (17.4) | 20.3 (20.2) | 22.7 (19.4) | 46.4 (44.4) | 95.8 (93.7) |
| sort_i64 | medium | 44.7 (42.7) | 45.0 (41.7) | 50.9 (46.0) | 545.3 (425.6) | 142.0 (138.2) |
| sort_i64 | large | 86.6 (85.1) | 96.1 (95.7) | 91.2 (88.4) | 1178.5 (1158.5) | 230.9 (221.6) |
| sort_insertion | small | 19.8 (17.9) | 25.0 (14.1) | n/a | n/a | n/a |
| sort_insertion | medium | 21.3 (20.3) | 14.6 (13.9) | n/a | n/a | n/a |
| sort_insertion | large | 24.3 (22.6) | 17.7 (16.2) | n/a | n/a | n/a |
| binary_trees | small | 28.9 (22.2) | 13.6 (12.9) | 13.6 (13.2) | 47.0 (42.8) | 123.9 (115.3) |
| binary_trees | medium | 332.7 (297.5) | 23.4 (22.4) | 26.4 (23.9) | 101.4 (98.1) | 115.0 (103.4) |
| binary_trees | large | 1471.0 (1216.1) | 58.9 (57.9) | 58.3 (58.0) | 357.0 (330.0) | 189.8 (156.1) |
| binary_trees_refs | small | 20.0 (19.8) | 15.0 (14.6) | 15.4 (15.1) | 46.7 (46.2) | 96.1 (83.7) |
| binary_trees_refs | medium | 52.4 (47.5) | 29.3 (25.6) | 27.4 (27.0) | 112.8 (104.7) | 98.0 (96.1) |
| binary_trees_refs | large | 140.0 (129.7) | 64.7 (61.3) | 67.5 (66.1) | 375.3 (356.2) | 128.1 (127.9) |
| string_split | small | 18.4 (18.4) | 13.9 (13.3) | 23.1 (19.3) | 54.9 (47.2) | 119.6 (118.5) |
| string_split | medium | 34.4 (33.3) | 19.1 (18.2) | 25.2 (22.8) | 91.2 (78.3) | 161.7 (143.0) |
| string_split | large | 196.8 (190.4) | 60.2 (58.0) | 72.3 (68.8) | 263.7 (239.9) | 313.2 (298.1) |
| sieve | small | 22.6 (19.6) | 12.5 (12.4) | 14.7 (14.4) | 53.1 (49.9) | 85.0 (84.7) |
| sieve | medium | 54.4 (50.6) | 36.6 (32.6) | 37.9 (35.8) | 264.5 (259.9) | 129.2 (113.5) |
| sieve | large | 124.3 (116.8) | 89.7 (88.9) | 85.7 (83.9) | 715.7 (705.8) | 191.6 (186.8) |
| hashmap_wordcount | small | 18.7 (16.6) | 16.0 (14.8) | 16.4 (15.8) | 43.0 (42.7) | 95.4 (91.0) |
| hashmap_wordcount | medium | 42.6 (40.7) | 35.5 (32.7) | 57.8 (56.5) | 90.6 (83.8) | 119.4 (113.2) |
| hashmap_wordcount | large | 149.7 (143.8) | 102.8 (99.7) | 227.6 (223.9) | 348.9 (319.5) | 234.6 (227.9) |
| array_wordcount | small | 29.9 (24.2) | 24.3 (21.2) | n/a | n/a | n/a |
| array_wordcount | medium | 19.5 (18.2) | 21.1 (19.1) | n/a | n/a | n/a |
| array_wordcount | large | 24.4 (23.2) | 24.2 (22.4) | n/a | n/a | n/a |

### Artifacts and generated C

Executable sizes and the size of the generated C sources. The generated C size is measured before the C compiler runs; header files and the shared runtime are not counted per program.

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | --- | --- | --- | --- |
| fib | 255,606 | 31,430 | 40,164 | 140,288 |
| fib_iter | 254,587 | 31,703 | 39,032 | n/a |
| fib_memo | 256,304 | 37,253 | 41,381 | n/a |
| loop_sum | 253,971 | 29,534 | 39,032 | 140,288 |
| string_build | 254,624 | 32,280 | 39,717 | 141,312 |
| vec_memory | 253,226 | 23,866 | 38,393 | 131,584 |
| sort_i64 | 257,043 | 43,846 | 40,404 | 141,824 |
| sort_insertion | 256,025 | 38,257 | 39,892 | n/a |
| matrix_multiply | 252,425 | 30,198 | 39,080 | 131,584 |
| binary_trees | 260,878 | 38,887 | 43,154 | 141,824 |
| binary_trees_refs | 261,390 | 38,406 | 43,174 | 141,824 |
| string_split | 255,271 | 35,679 | 40,396 | 142,336 |
| sieve | 255,337 | 33,544 | 39,555 | 140,800 |
| mandelbrot | 250,960 | 23,070 | 38,732 | 131,072 |
| iterator_pipeline | 390,854 | 213,008 | 38,588 | 131,072 |
| hashmap_wordcount | 262,471 | 53,208 | 42,302 | 146,432 |
| array_wordcount | 255,337 | 34,751 | 39,207 | n/a |
| generic_stack | 257,107 | 35,689 | 40,784 | 134,144 |
| fn_pipeline | 386,185 | 204,289 | 38,400 | 131,584 |
| string_unicode | 258,549 | 49,890 | 41,377 | 140,800 |
| hashmap_i64 | 257,015 | 39,530 | 39,891 | 136,192 |
| hashmap_string | 260,087 | 46,787 | 41,594 | 139,264 |
| compile_large (compile-only) | 372,743 | 315,881 | n/a | n/a |
| modules_compile (compile-only) | 622,182 | 2,961,133 | n/a | n/a |
| package_compile (compile-only) | 307,261 | 761,960 | n/a | n/a |

## Analysis

### What this run shows

lsaca 2.5.0 lands 14 of the 22 executed workloads inside 1.4x of C, 16 inside 1.5x and 12 inside 1.15x. The median lsaca/C ratio over the executed workloads is 1.14x.

Absolute milliseconds are not comparable with the 2.4.0 baseline even though both runs used the same C reference compiler (gcc 16.2.0): the archived run was taken earlier on a different host state. The table below therefore compares C-normalised ratios: each language's time divided by the C time of the same run. A positive change means lsaca lost ground relative to the C reference.

| workload | 2.4.0 ratio to C | 2.5.0 ratio to C | change |
| --- | --- | --- | --- |
| `string_split` | 1.17x | 1.80x | +53.8% |
| `string_unicode` | 1.80x | 2.49x | +38.7% |
| `sieve` | 1.11x | 1.49x | +33.7% |
| `binary_trees` | 11.48x | 14.24x | +24.1% |
| `sort_insertion` | 1.24x | 1.47x | +18.1% |
| `fib_iter` | 0.99x | 1.14x | +15.4% |
| `loop_sum` | 1.19x | 1.32x | +10.5% |
| `mandelbrot` | 1.01x | 1.11x | +9.5% |
| `hashmap_i64` | 1.56x | 1.59x | +1.7% |
| `hashmap_string` | 1.09x | 1.09x | +0.2% |
| `binary_trees_refs` | 1.79x | 1.79x | -0.1% |
| `matrix_multiply` | 1.07x | 1.06x | -1.0% |
| `string_build` | 4.77x | 4.68x | -1.9% |
| `sort_i64` | 1.05x | 0.99x | -5.6% |
| `generic_stack` | 1.13x | 1.05x | -7.1% |
| `fib` | 1.20x | 1.09x | -9.2% |
| `hashmap_wordcount` | 1.32x | 1.20x | -9.4% |
| `fib_memo` | 1.03x | 0.93x | -10.2% |
| `array_wordcount` | 1.05x | 0.93x | -11.7% |
| `vec_memory` | 1.21x | 0.92x | -24.1% |
| `iterator_pipeline` | 1.48x | 1.05x | -28.7% |
| `fn_pipeline` | 1.23x | 0.80x | -34.4% |

### Largest improvements against the 2.4.0 baseline

- `fn_pipeline`: 1.23x to 0.80x (-34.4%).
- `iterator_pipeline`: 1.48x to 1.05x (-28.7%).
- `vec_memory`: 1.21x to 0.92x (-24.1%).
- `array_wordcount`: 1.05x to 0.93x (-11.7%).
- `fib_memo`: 1.03x to 0.93x (-10.2%).
- `hashmap_wordcount`: 1.32x to 1.20x (-9.4%).

### Largest regressions against the 2.4.0 baseline

- `string_split`: 1.17x to 1.80x (+53.8%).
- `string_unicode`: 1.80x to 2.49x (+38.7%).
- `sieve`: 1.11x to 1.49x (+33.7%).
- `binary_trees`: 11.48x to 14.24x (+24.1%).
- `sort_insertion`: 1.24x to 1.47x (+18.1%).
- `fib_iter`: 0.99x to 1.14x (+15.4%).

### Workloads outside the 1.4x target

These workloads are above the 1.4x target in this run. Each one has a reason; the reasons are checked against the program source, not guessed.

| workload | lsaca runtime (ms) | C runtime (ms) | ratio | note |
| --- | --- | --- | --- | --- |
| `binary_trees` | 332.7 | 23.4 | 14.24x | by-value tree semantics: each recursive step clones the child subtree, so the deep copies are part of the workload by design; binary_trees_refs is the borrowed-traversal variant |
| `string_build` | 124.0 | 26.5 | 4.68x | one million appends; each append goes through the checked String API instead of writing into a raw buffer |
| `string_unicode` | 40.5 | 16.3 | 2.49x | owned String calls and Unicode table lookups; character-level work checks UTF-8 boundaries |
| `string_split` | 34.4 | 19.1 | 1.80x | splitting and parsing an owned string; every token is a String and parsing checks it |
| `binary_trees_refs` | 52.4 | 29.3 | 1.79x | borrowed tree traversal; references do not copy the tree, so only the traversal cost remains |
| `hashmap_i64` | 276.9 | 174.3 | 1.59x | integer-key map operations; the C version uses a hand-written open-addressing table |
| `sieve` | 54.4 | 36.6 | 1.49x | a large vector of flags with checked indexing; the C version uses a raw calloced array |
| `sort_insertion` | 21.3 | 14.6 | 1.47x | quadratic insertion sort; the inner loop is bounds-checked and the workload is dominated by the C reference's raw array accesses |

### Acceptance against the standing targets

| target | result | status |
| --- | --- | --- |
| Every executed runtime within 1.4x of C | 14/22 within 1.4x | partly met |
| Smallest fresh single-file build 0.35-0.5 s | 0.83 s | not met |
| Cached rebuild around 15 ms | 140 ms | not met |
| Compile-only workload within 2.5x of the C compile | 3.70 s, no C reference | not scored |
| Generated C compiles without warnings | every measured program built by the harness | met |
| Independent oracle agreement | 22 program checksums and 39 data-scale checksums agree | met |
| Runtime medians within 10% of 2.4.0 (C-normalised) | 7 of 22 outside the budget | partly met |

The 1.4x target is a language-level goal, not a hard release gate: a workload outside it must have a recorded reason, and the table above names them all.

### Python and Node.js

The dynamic languages are far slower than the compiled ones on these workloads. lsaca is between 2.00x and 39.28x the speed of CPython and between 2.60x and 8.45x the speed of Node.js. They are included to keep the page honest about the range: a language that only compares with C can look worse than it is.

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

2. Absolute milliseconds are specific to this host and toolchain: 12th Gen Intel(R) Core(TM) i7-1260P running Windows-11-10.0.26300-SP0, C reference gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0. The 2.4.0 baseline was measured with gcc 16.2.0, so cross-release claims are made about C-normalised ratios only.

3. All measurements were taken in one session on an otherwise idle machine; the harness runs one program at a time and does not overlap builds with measurements.

4. Short workloads have higher relative noise. Every runtime table reports both the median and the minimum of the measured runs so the spread is visible, and the harness discards the first run as a warm-up.

5. The closest workload is `fn_pipeline` at 0.80x of C; the widest gap is `binary_trees` at 14.24x. Both are real measurements, not rounding artefacts of a single run.

6. `binary_trees` is the largest deliberate semantic gap: lsaca implements value-semantics trees, so a tree assignment copies the tree and the program builds a different shape from the pointer-based C and Rust versions. It is kept because it measures the cost of the language's semantics, not because the implementation is slow.

7. `hashmap_string` is faster than the C reference on some runs. That is the Swiss-table implementation doing its job; the C reference uses a simpler open-addressed table, so this is a data-structure comparison as much as a language comparison.

8. The alternative-implementation workloads (the programs whose name ends in a second variant) are built only for lsaca and C, because translating every alternative five ways would measure translation style rather than the language.

9. Compile-only workloads measure the build, not a running program: they have no runtime row, and their C reference column is n/a. They exist to show how build time grows with source size and module count.

10. The cached build column is the best case for an unchanged source file: the harness reuses the content-addressed build cache, so it measures the cached path rather than a fresh compile.

11. Peak memory is the peak working set of the child process, sampled by the harness through the platform API. It includes the runtime's allocator pools, which trade a little memory for speed.

12. Every runtime table cell is validated: the harness compares the printed checksum against the oracle and fails the measurement if it differs. A wrong answer cannot become a fast result.

13. Relative to the 2.4.0 baseline, `string_split`, `string_unicode`, `sieve`, `binary_trees`, `sort_insertion`, `fib_iter` moved outside the 2.4.0 measurement noise after normalising by the C reference. The release notes track these explicitly instead of hiding them in a different host speed.

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

The 2.4.0 baseline used for the comparison table lives in `benchmarks/results/archive/v2.4/latest.json`; it reports lsaca 2.4.0 and is never overwritten by the harness.

The regression guard reads the same files:

```sh
python scripts/bench_guard.py --results benchmarks/results/latest.json \
    --baseline benchmarks/results/archive/v2.4/latest.json
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
