# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
guesses. Every number below was produced by the benchmark harness in this repository on a
single machine, on the same run, with every program compiled by the same compiler. The run
measured `25` programs (22 executed, 3 compile-only),
each workload implemented in lsaca plus the reference languages it is compared with (C, Rust,
Python 3 and Node.js).

> **Version note.** The tables on this page are the official **2.2.0** run, produced on
> 2026-11-07 (harness timestamp 17:21 +08:00). Every number was measured by the benchmark
> harness in this repository on a single host with the release compiler `lsaca 2.2.0`
> (repository commit `97280dd`, gcc 15.2.0 for the C reference builds). The previous
> published run (2.1.0, 2026-11-06) is the baseline for the explicit comparison in the
> analysis; the 2.0.0 run (2026-11-03) remains the older reference point.
>
> **Host drift.** The host used for this run was about 12% slower overall than the host used
> for the 2.1.0 run (mean lsaca runtime +12%, median +14%; the C builds moved by more). Every
> language is measured in the same run, so the ratios between languages are the portable
> part; do not compare absolute milliseconds across the two published runs.

The headline is that lsaca compiles to C that runs, on average, close to hand-written C:
9 of the 22 executed workloads are within 1.4x of the C build in this run (14 within 1.5x,
5 within 1.15x), and one workload is actually faster than its C build: `hashmap_string` at 0.92x.
The remaining workloads are either on the documented list of known gaps or are
semantic exceptions where the workload itself performs by-value copies. `binary_trees`
remains the largest gap at 8.17x C but is a by-value-copy workload by design; among the
ordinary workloads `string_build` is the largest at 6.83x, while `string_split`, `sort_i64`,
`fib_iter`, `mandelbrot` and `sieve` all improved their ratio against the previous run. Build
time is not yet where it should be: a cold single-file build takes 0.48-2.07 s (median 0.64 s,
against a median gcc build of 0.29 s), a cached rebuild 60-122 ms (median 72 ms).

## Workloads

Twenty-five programs are measured. Twenty-two are executed; the other three
(`compile_large`, `modules_compile` and `package_compile`) are compile-only workloads that
exist to measure build time on a larger source file, a multi-module program and a package with
path dependencies. Four of the executed programs are alternative implementations of another
workload — `fib_iter` and `fib_memo` against `fib`, `sort_insertion` against `sort_i64`, and
`array_wordcount` against `hashmap_wordcount` — and are built and reported only in lsaca and C.

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
| `compile_large` | compile-time scaling with source size | generated 4,511-line lsaca source (450 functions); **compile-only**, never executed by the harness | `checksum=14655037,` (not measured) |
| `modules_compile` | compile-time scaling with a multi-file crate | one crate root (`main.ls`) plus eight sibling module files, 3,592 lines across 9 files, with structs, enums, generics, trait impls, strings, vectors and hash maps; **compile-only**, never executed by the harness | `checksum=547598677,` (not measured) |
| `package_compile` | compile-time scaling with path dependencies | binary package `compile_app` plus library packages `util_a` and `util_b` (`util_b` depends on `util_a`), 1,088 lines across 13 files in 3 packages; **compile-only**, never executed by the harness | `checksum=417367897,` (not measured) |

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

The measurements in this page come from one host. Everything in the table below was collected by
the harness at run time and is stored in `benchmarks/results/latest.json`, together with the
individual run times.

| field | value |
| --- | --- |
| Measured at | 2026-11-07 17:21:34 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 2.2.0, repository HEAD `97280dd`, compiler sha256 prefix `1c74d15ecf9a1b49`, compiler mtime 2026-11-07 17:20:48 +08:00 |
| lsaca build path | `lsaca build -r`, default direct C compiler back end (gcc 15.2.0, `-O2 -DNDEBUG`); the old CMake + Ninja path is available as `--cmake` |
| C compiler | gcc 15.2.0 (MSYS2 UCRT64, Rev14, Built by MSYS2 project) |
| Rust compiler | rustc 1.99.0 (`b940084d7 2026-09-28`) |
| Python | 3.14.4 |
| Node.js | v24.14.1 |
| Harness | 25 programs (22 executed), 1 warm-up + 7 runs, 3 compile repeats, 3 scales at 3 runs |

## Results

All times are milliseconds unless stated otherwise. Every row in a table was measured on the
same host in the same run; numbers from different runs on different hosts are never mixed in a
single table.

### Compile time

Cold build means the whole pipeline from lsaca source to an executable with an empty cache; cached build re-uses the compile cache and only re-links. Times are medians of 3 compile repeats, in seconds unless stated otherwise.

| program | lsaca cold build (default back end) | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| fib | 1.081 | 0.083 | n/a | 0.346 | 0.712 | n/a | n/a |
| fib_iter | 0.668 | 0.069 | n/a | 0.334 | n/a | n/a | n/a |
| fib_memo | 0.728 | 0.072 | n/a | 0.333 | n/a | n/a | n/a |
| loop_sum | 0.582 | 0.072 | n/a | 0.442 | 0.725 | n/a | n/a |
| string_build | 0.791 | 0.119 | n/a | 0.292 | 0.435 | n/a | n/a |
| vec_memory | 0.515 | 0.073 | n/a | 0.270 | 0.309 | n/a | n/a |
| sort_i64 | 0.609 | 0.081 | n/a | 0.347 | 0.583 | n/a | n/a |
| sort_insertion | 0.603 | 0.069 | n/a | 0.288 | n/a | n/a | n/a |
| matrix_multiply | 0.515 | 0.067 | n/a | 0.279 | 0.336 | n/a | n/a |
| binary_trees | 0.578 | 0.061 | n/a | 0.379 | 0.448 | n/a | n/a |
| binary_trees_refs | 0.662 | 0.066 | n/a | 0.312 | 0.427 | n/a | n/a |
| string_split | 0.535 | 0.065 | n/a | 0.252 | 0.421 | n/a | n/a |
| sieve | 0.685 | 0.065 | n/a | 0.262 | 0.423 | n/a | n/a |
| mandelbrot | 0.480 | 0.064 | n/a | 0.255 | 0.252 | n/a | n/a |
| iterator_pipeline | 2.069 | 0.076 | n/a | 0.270 | 0.349 | n/a | n/a |
| hashmap_wordcount | 0.940 | 0.122 | n/a | 0.366 | 0.560 | n/a | n/a |
| array_wordcount | 0.626 | 0.064 | n/a | 0.292 | n/a | n/a | n/a |
| generic_stack | 0.667 | 0.088 | n/a | 0.291 | 0.366 | n/a | n/a |
| fn_pipeline | 1.969 | 0.073 | n/a | 0.284 | 0.305 | n/a | n/a |
| string_unicode | 0.797 | 0.095 | n/a | 0.437 | 0.449 | n/a | n/a |
| hashmap_i64 | 0.505 | 0.060 | n/a | 0.273 | 0.546 | n/a | n/a |
| hashmap_string | 0.625 | 0.075 | n/a | 0.311 | 0.699 | n/a | n/a |
| compile_large (compile-only) | 2.367 | 0.662 | n/a | n/a | n/a | n/a | n/a |
| modules_compile (compile-only) | 4.210 | 0.814 | n/a | n/a | n/a | n/a | n/a |
| package_compile (compile-only) | 1.615 | 0.174 | n/a | n/a | n/a | n/a | n/a |

Cold builds of the 22 executed programs take 0.48-2.07 s. Excluding the two large pipeline programs (`iterator_pipeline` 2.07 s, `fn_pipeline` 1.97 s), the ordinary single-file programs span 0.48-1.08 s. Cached rebuilds take 60-122 ms.

gcc itself needs 0.25-0.44 s per program (median 0.29 s), so on Windows a cold build is dominated by gcc compiling the generated C and the shared runtime translation units, not by the lsaca front end. The three compile-only workloads show the scaling: `compile_large` needs 2.37 s cold and 662 ms cached for 301,361 bytes (0.30 MB) of generated C; `modules_compile` needs 4.21 s / 814 ms for about 3,600 lines across 9 files; `package_compile` needs 1.62 s / 174 ms for about 1,090 lines across 13 files in 3 packages.

Against the 2.1.0 run, cold builds moved with the host: the median single-file cold build went from 0.56 s to 0.64 s and the median gcc build of the same programs from 0.26 s to 0.29 s, so most of the movement is the slower host rather than new compiler work. The compile-only workloads grew too (`compile_large` 1.96 -> 2.37 s, `modules_compile` 2.96 -> 4.21 s, `package_compile` 1.33 -> 1.61 s), again alongside a slower host; the generated C byte counts are unchanged (301,361 bytes for `compile_large`). Cached builds measured 60-122 ms (median 72 ms) versus 52.5-108.2 ms (median 62 ms) in the 2.1.0 run.

### Runtime

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 37.4 (34.0) | 16.1 (15.6) | 17.2 (16.6) | 27.6 (23.9) | 108.4 (99.4) | 23.0 (20.6) | 43.6 (40.4) | 14.5 (14.0) | 18.0 (16.6) | 192.9 (189.0) | 36.8 (35.4) | 25.9 (25.1) | 36.0 (34.1) | 34.2 (33.8) | 20.1 (19.3) | 49.3 (43.5) | 19.0 (17.4) | 25.2 (23.1) | 15.5 (15.0) | 42.7 (40.6) | 234.4 (220.9) | 227.5 (224.0) |
| C (gcc -O2) | 30.5 (27.7) | 14.0 (12.8) | 11.6 (11.0) | 22.6 (17.5) | 15.9 (14.4) | 16.4 (15.7) | 30.9 (28.8) | 10.0 (9.2) | 12.2 (11.7) | 23.6 (21.0) | 20.8 (19.7) | 12.9 (12.4) | 30.0 (28.8) | 32.0 (28.7) | 18.1 (17.6) | 28.6 (27.0) | 10.6 (9.8) | 22.0 (20.6) | 13.0 (12.0) | 14.9 (13.3) | 148.8 (144.2) | 246.2 (170.5) |
| Rust (rustc -O) | 25.4 (22.5) | n/a | n/a | 22.7 (18.9) | 74.5 (67.2) | 16.9 (15.8) | 32.5 (30.8) | n/a | 12.1 (11.8) | 24.4 (22.2) | 24.2 (23.4) | 13.2 (12.4) | 28.2 (26.7) | 30.1 (28.8) | 20.5 (19.1) | 53.7 (52.1) | n/a | 19.6 (19.1) | 13.3 (12.7) | 15.1 (13.8) | 200.7 (188.9) | 256.8 (249.5) |
| Python 3 | 1273.7 (1145.5) | n/a | n/a | 1138.7 (1057.9) | 450.6 (411.9) | 549.2 (503.9) | 761.8 (629.9) | n/a | 1306.9 (1165.6) | 105.5 (98.8) | 119.8 (107.8) | 64.3 (60.7) | 546.6 (523.6) | 2523.5 (2367.7) | 231.5 (227.3) | 115.5 (109.9) | n/a | 364.1 (341.7) | 41.8 (39.4) | 240.6 (226.7) | 2118.3 (1887.5) | 1031.6 (994.4) |
| Node.js | 307.2 (262.5) | n/a | n/a | 177.0 (148.1) | 266.1 (235.5) | 268.1 (231.4) | 224.3 (212.7) | n/a | 139.2 (136.6) | 155.6 (144.6) | 149.2 (143.5) | 148.2 (134.0) | 186.7 (156.0) | 166.5 (160.0) | 156.9 (151.0) | 174.9 (152.3) | n/a | 152.0 (149.3) | 128.9 (124.5) | 174.8 (161.7) | 665.8 (639.1) | 373.4 (357.8) |

Median of 7 measured runs after one warm-up, in milliseconds, with the minimum in parentheses.

The next table divides each language median by the C median for the same program. A value below 1.00x means that language was faster than the C build in this run.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 1.23x | 1.15x | 1.48x | 1.22x | 6.83x | 1.40x | 1.41x | 1.45x | 1.47x | 8.17x | 1.77x | 2.00x | 1.20x | 1.07x | 1.11x | 1.72x | 1.79x | 1.15x | 1.19x | 2.88x | 1.58x | 0.92x |
| Rust | 0.83x | n/a | n/a | 1.00x | 4.70x | 1.04x | 1.05x | n/a | 0.99x | 1.03x | 1.16x | 1.02x | 0.94x | 0.94x | 1.13x | 1.87x | n/a | 0.89x | 1.02x | 1.02x | 1.35x | 1.04x |
| Python 3 | 41.74x | n/a | n/a | 50.36x | 28.40x | 33.59x | 24.62x | n/a | 107.17x | 4.47x | 5.76x | 4.97x | 18.21x | 78.78x | 12.76x | 4.03x | n/a | 16.57x | 3.20x | 16.20x | 14.24x | 4.19x |
| Node.js | 10.07x | n/a | n/a | 7.83x | 16.77x | 16.40x | 7.25x | n/a | 11.41x | 6.59x | 7.18x | 11.46x | 6.22x | 5.20x | 8.65x | 6.11x | n/a | 6.92x | 9.88x | 11.77x | 4.48x | 1.52x |

### Peak memory

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 5.1 | 5.1 | 5.1 | 5.1 | 11.7 | 20.1 | 7.9 | 5.2 | 5.5 | 53.1 | 17.1 | 10.6 | 20.1 | 5.1 | 12.5 | 5.5 | 5.1 | 13.9 | 5.2 | 6.4 | 56.6 | 30.8 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.0 | 4.4 | 7.7 | 7.7 | 4.1 | 18.8 | 4.0 | 11.2 | 4.2 | 4.1 | 12.6 | 4.0 | 4.3 | 52.5 | 19.7 |
| Rust (rustc -O) | 4.2 | n/a | n/a | 4.2 | 7.4 | 18.9 | 5.2 | n/a | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 | 11.3 | 4.4 | n/a | 12.7 | 4.1 | 4.7 | 55.9 | 19.8 |
| Python 3 | 11.0 | n/a | n/a | 10.9 | 74.5 | 142.8 | 20.4 | n/a | 13.8 | 21.0 | 21.1 | 17.1 | 26.2 | 11.0 | 49.2 | 11.4 | n/a | 54.2 | 11.1 | 25.6 | 113.5 | 38.0 |
| Node.js | 42.9 | n/a | n/a | 44.9 | 88.4 | 121.3 | 51.6 | n/a | 47.1 | 56.4 | 56.5 | 50.9 | 59.0 | 44.6 | 58.0 | 46.6 | n/a | 72.3 | 43.1 | 69.2 | 102.7 | 90.6 |

Peak working set of the child process, sampled by the harness while it runs. lsaca programs track their allocations, so a program that never leaks memory settles at a peak close to the C build's. The by-value `binary_trees` row is the peak-memory counterpart of its runtime row: the deep copies are part of the workload by design.

### Runtime by data scale

The argument-driven workloads are measured at three sizes. Each cell is the median in milliseconds at that scale.

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| fib | small | 20.1 (19.5) | 10.3 (9.9) | 20.4 (19.7) | 70.9 (64.6) | 220.6 (215.1) |
| fib | medium | 37.4 (34.0) | 30.5 (27.7) | 25.4 (22.5) | 1273.7 (1145.5) | 307.2 (262.5) |
| fib | large | 44.3 (44.1) | 36.9 (35.5) | 21.2 (20.6) | 2192.4 (1750.6) | 541.3 (512.4) |
| fib_iter | small | 15.9 (15.6) | 13.6 (13.1) | n/a | n/a | n/a |
| fib_iter | medium | 16.1 (15.6) | 14.0 (12.8) | n/a | n/a | n/a |
| fib_iter | large | 17.4 (16.4) | 13.5 (12.8) | n/a | n/a | n/a |
| fib_memo | small | 16.6 (16.3) | 11.6 (11.5) | n/a | n/a | n/a |
| fib_memo | medium | 17.2 (16.6) | 11.6 (11.0) | n/a | n/a | n/a |
| fib_memo | large | 16.1 (16.0) | 12.0 (11.3) | n/a | n/a | n/a |
| loop_sum | small | 26.1 (25.1) | 18.5 (18.1) | 18.5 (16.7) | 225.6 (204.5) | 139.8 (135.1) |
| loop_sum | medium | 27.6 (23.9) | 22.6 (17.5) | 22.7 (18.9) | 1138.7 (1057.9) | 177.0 (148.1) |
| loop_sum | large | 22.8 (22.2) | 22.2 (20.7) | 22.4 (18.8) | 1676.0 (1640.1) | 173.8 (163.4) |
| string_build | small | 15.8 (13.9) | 10.0 (9.9) | 12.6 (12.0) | 42.0 (40.0) | 189.4 (180.0) |
| string_build | medium | 108.4 (99.4) | 15.9 (14.4) | 74.5 (67.2) | 450.6 (411.9) | 266.1 (235.5) |
| string_build | large | 330.8 (329.1) | 27.8 (25.4) | 293.3 (264.4) | 1560.1 (1516.5) | 607.1 (500.6) |
| sort_i64 | small | 17.0 (15.7) | 14.5 (11.5) | 9.8 (9.5) | 41.3 (40.3) | 176.2 (160.4) |
| sort_i64 | medium | 43.6 (40.4) | 30.9 (28.8) | 32.5 (30.8) | 761.8 (629.9) | 224.3 (212.7) |
| sort_i64 | large | 82.2 (81.7) | 68.0 (66.5) | 63.5 (62.5) | 1844.5 (1834.9) | 260.7 (231.9) |
| sort_insertion | small | 13.8 (13.6) | 10.7 (10.6) | n/a | n/a | n/a |
| sort_insertion | medium | 14.5 (14.0) | 10.0 (9.2) | n/a | n/a | n/a |
| sort_insertion | large | 17.3 (17.1) | 12.4 (12.3) | n/a | n/a | n/a |
| binary_trees | small | 15.5 (14.8) | 13.1 (13.0) | 12.5 (12.3) | 38.2 (37.1) | 148.8 (142.2) |
| binary_trees | medium | 192.9 (189.0) | 23.6 (21.0) | 24.4 (22.2) | 105.5 (98.8) | 155.6 (144.6) |
| binary_trees | large | 1044.1 (920.3) | 52.5 (51.1) | 55.7 (55.5) | 308.6 (305.8) | 202.5 (202.0) |
| binary_trees_refs | small | 16.8 (16.4) | 9.9 (9.3) | 13.3 (13.2) | 51.0 (48.2) | 127.0 (125.3) |
| binary_trees_refs | medium | 36.8 (35.4) | 20.8 (19.7) | 24.2 (23.4) | 119.8 (107.8) | 149.2 (143.5) |
| binary_trees_refs | large | 96.1 (95.9) | 52.9 (50.0) | 59.8 (56.4) | 391.0 (301.0) | 159.4 (159.0) |
| string_split | small | 14.9 (14.7) | 9.8 (9.1) | 10.9 (10.6) | 33.1 (32.8) | 149.7 (146.9) |
| string_split | medium | 25.9 (25.1) | 12.9 (12.4) | 13.2 (12.4) | 64.3 (60.7) | 148.2 (134.0) |
| string_split | large | 138.0 (136.5) | 44.3 (42.0) | 43.3 (41.2) | 342.5 (323.1) | 323.5 (316.7) |
| sieve | small | 16.5 (16.3) | 10.9 (10.7) | 11.7 (10.6) | 65.4 (62.6) | 151.6 (150.2) |
| sieve | medium | 36.0 (34.1) | 30.0 (28.8) | 28.2 (26.7) | 546.6 (523.6) | 186.7 (156.0) |
| sieve | large | 89.6 (78.7) | 78.2 (75.1) | 70.9 (67.5) | 1236.3 (1224.2) | 269.8 (241.5) |
| hashmap_wordcount | small | 30.6 (26.7) | 23.5 (19.9) | 16.1 (15.9) | 43.9 (41.7) | 152.0 (145.3) |
| hashmap_wordcount | medium | 49.3 (43.5) | 28.6 (27.0) | 53.7 (52.1) | 115.5 (109.9) | 174.9 (152.3) |
| hashmap_wordcount | large | 171.4 (167.6) | 120.7 (114.7) | 188.1 (182.9) | 454.4 (437.4) | 254.6 (247.2) |
| array_wordcount | small | 16.5 (15.9) | 10.6 (10.5) | n/a | n/a | n/a |
| array_wordcount | medium | 19.0 (17.4) | 10.6 (9.8) | n/a | n/a | n/a |
| array_wordcount | large | 21.5 (18.6) | 10.9 (10.5) | n/a | n/a | n/a |

At every scale the C and lsaca columns track closely for the scalar, map and string workloads; the gap widens where the workload allocates more, and it grows with size for `string_build` (up to 11.88x C at the largest scale) and `binary_trees` (up to 19.90x C) because each element costs an allocation or a by-value copy. The full scale matrix is the fastest way to check that a change helps the whole curve rather than one default size, and the per-scale results are stored under `programs.<name>.scales` in the results JSON.

### Artifacts and generated C

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | ---: | ---: | ---: | ---: |
| fib | 335,640 | 16,911 | 141,002 | 141,312 |
| fib_iter | 334,621 | 17,184 | 139,870 | n/a |
| fib_memo | 336,338 | 22,700 | 142,044 | n/a |
| loop_sum | 334,517 | 15,015 | 139,870 | 141,312 |
| string_build | 334,658 | 17,761 | 140,382 | 142,336 |
| vec_memory | 333,260 | 9,347 | 139,058 | 132,096 |
| sort_i64 | 337,077 | 29,327 | 140,557 | 142,336 |
| sort_insertion | 336,059 | 23,738 | 140,557 | n/a |
| matrix_multiply | 332,459 | 15,679 | 139,745 | 132,608 |
| binary_trees | 339,888 | 24,368 | 143,819 | 142,336 |
| binary_trees_refs | 340,400 | 23,887 | 143,839 | 142,336 |
| string_split | 335,305 | 21,160 | 142,849 | 142,848 |
| sieve | 335,371 | 19,025 | 140,045 | 141,824 |
| mandelbrot | 330,994 | 8,550 | 139,058 | 131,072 |
| iterator_pipeline | 420,445 | 329,216 | 139,253 | 132,096 |
| hashmap_wordcount | 340,189 | 30,562 | 143,556 | 145,920 |
| array_wordcount | 335,371 | 20,232 | 140,045 | n/a |
| generic_stack | 337,653 | 21,170 | 142,725 | 133,632 |
| fn_pipeline | 415,258 | 310,306 | 139,238 | 132,096 |
| string_unicode | 339,095 | 35,371 | 142,035 | 140,800 |
| hashmap_i64 | 336,853 | 24,196 | 140,381 | 136,192 |
| hashmap_string | 339,916 | 31,417 | 143,360 | 138,240 |
| compile_large (compile-only) | 452,777 | 301,361 | n/a | n/a |
| modules_compile (compile-only) | 647,429 | 2,327,016 | n/a | n/a |
| package_compile (compile-only) | 387,602 | 681,780 | n/a | n/a |

Generated C is larger than the hand-written C source because it contains the whole program prelude (runtime declarations, the collections and string ABI, closure shims) in addition to the compiled body; since 2.0.0 the standard library is embedded into that generated C as well. The compile-only workloads are measured for build time only and are not executed by the harness.

## Analysis

### What 2.2.0 changes

2.2.0 is the second increment on top of the 2.1.0 Rust alignment. It adds fixed-length arrays
(`[T; N]`), loop labels (`'outer: loop` with `break 'outer` and `continue 'outer`), lightweight
lifetimes for reference fields (`struct View<'a> { data: &'a [T] }`), the `From` trait with `?`
conversions and the default `Error::source` method, borrowed collection accessors
(`HashMap::get`/`get_mut` with `&K`, `Vec::get_mut`/`first_mut`/`last_mut` and the matching
`[T]` methods), and the `lsaca fmt` source formatter. None of this changes the runtime
representation: values are still ORC handles, `String` is still guaranteed-valid UTF-8,
generics are still monomorphized, and the `HashMap` is the same Swiss-table implementation.

The shape of the runtime table is therefore stable. 9 of the 22 executed workloads are
within 1.4x of C in this run (14 within 1.5x, 5 within 1.15x), and one workload is
faster than C: `hashmap_string` at 0.92x. That is one fewer within 1.4x than in the 2.1.0 run (10 of 22),
but this run was taken on a host that was about 12% slower overall: the mean lsaca runtime
moved +12.2% and the median +13.9% against the 2.1.0 run, while the C
reference builds moved +15.6% at the median. Absolute milliseconds therefore mix code
changes with host drift; the ratio columns are the portable comparison.

Three workloads improved their ratio to C clearly. `string_split` 2.75x -> 2.00x: its lsaca
time dropped 39.3 -> 25.9 ms (-34.2%) while the C build moved only
-9.5%, so this is a genuine code-generation improvement in string handling.
`sort_i64` 1.71x -> 1.41x: 56.6 -> 43.6 ms (-23.0%, C -6.6%),
the follow-up item listed on the 2.1.0 page now largely recovered. `binary_trees` 11.05x ->
8.17x, but that one is mostly the host: lsaca 212.5 -> 192.9 ms (-9.2%) while
its C build moved +22.8%. `string_build` also improved from 7.48x to 6.83x, again
because the C build moved more (+22.1%) than lsaca (+11.5%), not because the
format-into-push fusion lost in 2.1 came back.

A few ratios moved the other way on this host: `string_unicode` 2.44x -> 2.88x (lsaca
+12.3%, C -4.7%), `array_wordcount` 1.45x -> 1.79x, `binary_trees_refs` 1.46x ->
1.77x, `hashmap_wordcount` 1.50x -> 1.72x and `hashmap_i64` 1.35x -> 1.58x. In each case the
lsaca and C builds of the same program moved in opposite directions or by very different
amounts, which is what makes the ratio noisy at these run lengths rather than evidence of a
language-level regression.

| workload | 2.1.0 runtime (ms) | 2.2.0 runtime (ms) | change | 2.1.0 ratio to C | 2.2.0 ratio to C |
| --- | ---: | ---: | ---: | ---: | ---: |
| `binary_trees` | 212.5 | 192.9 | -9.2% | 11.05x | 8.17x |
| `string_build` | 97.2 | 108.4 | +11.5% | 7.48x | 6.83x |
| `string_unicode` | 38.1 | 42.7 | +12.3% | 2.44x | 2.88x |
| `string_split` | 39.3 | 25.9 | -34.2% | 2.75x | 2.00x |
| `array_wordcount` | 13.1 | 19.0 | +45.5% | 1.45x | 1.79x |
| `binary_trees_refs` | 38.2 | 36.8 | -3.6% | 1.46x | 1.77x |
| `hashmap_wordcount` | 32.5 | 49.3 | +51.8% | 1.50x | 1.72x |
| `hashmap_i64` | 196.3 | 234.4 | +19.4% | 1.35x | 1.58x |
| `fib_memo` | 13.6 | 17.2 | +26.6% | 1.33x | 1.48x |
| `matrix_multiply` | 16.2 | 18.0 | +11.1% | 1.27x | 1.47x |
| `sort_insertion` | 14.3 | 14.5 | +1.9% | 1.45x | 1.45x |
| `sort_i64` | 56.6 | 43.6 | -23.0% | 1.71x | 1.41x |
| `vec_memory` | 19.9 | 23.0 | +15.5% | 1.37x | 1.40x |
| `fib` | 27.6 | 37.4 | +35.2% | 1.17x | 1.23x |
| `loop_sum` | 20.6 | 27.6 | +34.2% | 1.54x | 1.22x |
| `sieve` | 37.8 | 36.0 | -4.7% | 1.29x | 1.20x |
| `fn_pipeline` | 12.1 | 15.5 | +28.1% | 1.46x | 1.19x |
| `fib_iter` | 24.1 | 16.1 | -33.2% | 2.63x | 1.15x |
| `generic_stack` | 17.2 | 25.2 | +46.1% | 1.24x | 1.15x |
| `iterator_pipeline` | 16.0 | 20.1 | +25.7% | 1.30x | 1.11x |
| `mandelbrot` | 39.5 | 34.2 | -13.5% | 1.25x | 1.07x |
| `hashmap_string` | 181.8 | 227.5 | +25.1% | 1.11x | 0.92x |

### The largest runtime changes

Compared with the published 2.1.0 run, the six workloads whose absolute time improved most were:

- `string_split`: 39.3 -> 25.9 ms (-34.2%, 2.00x C; C -9.5%)
- `fib_iter`: 24.1 -> 16.1 ms (-33.2%, 1.15x C; C +53.0%)
- `sort_i64`: 56.6 -> 43.6 ms (-23.0%, 1.41x C; C -6.6%)
- `mandelbrot`: 39.5 -> 34.2 ms (-13.5%, 1.07x C; C +1.1%)
- `binary_trees`: 212.5 -> 192.9 ms (-9.2%, 8.17x C; C +22.8%)
- `sieve`: 37.8 -> 36.0 ms (-4.7%, 1.20x C; C +2.4%)

The six that lost the most absolute time were:

- `hashmap_wordcount`: 32.5 -> 49.3 ms (+51.8%, 1.72x C; C +32.0%)
- `generic_stack`: 17.2 -> 25.2 ms (+46.1%, 1.15x C; C +58.1%)
- `array_wordcount`: 13.1 -> 19.0 ms (+45.5%, 1.79x C; C +17.9%)
- `fib`: 27.6 -> 37.4 ms (+35.2%, 1.23x C; C +28.9%)
- `loop_sum`: 20.6 -> 27.6 ms (+34.2%, 1.22x C; C +69.0%)
- `fn_pipeline`: 12.1 -> 15.5 ms (+28.1%, 1.19x C; C +57.3%)

Short workloads dominate both lists, and the C builds of the same programs moved by comparable
amounts in almost every case (see the C change column in the comparison table). Only
`string_split` and `sort_i64` show a large lsaca movement that the C build does not share, so
those are the two to read as code-generation changes. The rest were measured on a host that was
slower than the 2.1.0 host and should not be read as isolated compiler improvements or
regressions.

### Workloads outside the 1.4x target

9 of the 22 executed workloads are within 1.4x of the C build; 14 are within 1.5x. The
13 outside the 1.4x target are listed here, worst first (the by-value `binary_trees`
row is a semantic exception, not a performance gap):

| workload | 2.2.0 runtime (ms) | C runtime (ms) | ratio to C | note |
| --- | ---: | ---: | ---: | --- |
| `binary_trees` | 192.9 | 23.6 | 8.17x | by-value workload: each recursive step builds and returns a subtree by value, so the deep copies are part of the measured workload by design; the binary_trees_refs workload is the comparable reference-traversal variant |
| `string_build` | 108.4 | 15.9 | 6.83x | string growth and appends; the 2.1 and_str alignment removed the old format-into-push fusion and it has not been recovered |
| `string_unicode` | 42.7 | 14.9 | 2.88x | owned String wrapper calls and Unicode table lookups; the C build moved down on this host while the lsaca time moved up |
| `string_split` | 25.9 | 12.9 | 2.00x | splitting and parsing; improved from 2.75x to 2.00x |
| `array_wordcount` | 19.0 | 10.6 | 1.79x | flat-array alternative to hashmap_wordcount |
| `binary_trees_refs` | 36.8 | 20.8 | 1.77x | reference-based tree traversal; its C build moved down 20.6% on this host |
| `hashmap_wordcount` | 49.3 | 28.6 | 1.72x | hashing owned String words |
| `hashmap_i64` | 234.4 | 148.8 | 1.58x | integer-key map operations |
| `fib_memo` | 17.2 | 11.6 | 1.48x | memoized recursion |
| `matrix_multiply` | 18.0 | 12.2 | 1.47x | integer multiply-add loops |
| `sort_insertion` | 14.5 | 10.0 | 1.45x | quadratic insertion sort, alternative to the library sort |
| `sort_i64` | 43.6 | 30.9 | 1.41x | in-language sorting of random data; improved from 1.71x to 1.41x |
| `vec_memory` | 23.0 | 16.4 | 1.40x | allocation volume and peak memory; exactly on the 1.4x boundary (1.402x on the unrounded medians) |

The `binary_trees` workload is a semantic exception rather than a code-generation gap: the
recursive step builds and returns a subtree by value, so the deep copies are part of the
measured workload. Its `binary_trees_refs` sibling performs the same tree walk through
references and is the comparable measurement; it is now 1.77x C.

### Acceptance against the standing targets

| target | result | status |
| --- | --- | --- |
| every measured runtime within 1.4x of C | 9 of 22 executed programs are within 1.4x (14 within 1.5x, 5 within 1.15x); the 13 exceptions are tabulated above | partly met |
| smallest single-file cold build 0.35-0.5 s | 0.48 s (`mandelbrot`) | met |
| cached build ~15 ms | 60 ms for `hashmap_i64`; 60-122 ms across all programs | not met |
| `compile_large` cold build <= 2.5x the C reference | 2.37 s (301,361 bytes of generated C, 0.30 MB); the workload has no equivalent C build, so this is a snapshot and not a scored result | not scored |
| generated C warning-free under `-Wall -Wextra -Werror` | checked by every harness build in this run | met |
| expected output agrees with an independent oracle | 22 program checksums and 39 data-scale checksums agree | met |

Runtime and validation targets do not depend on host load to the same degree and are met.
Compile-time targets were set against an idle host; the cold and cached numbers above come from
one run on a host that was also running the rest of the measurement suite, so they are
indicative snapshots rather than scored results.

### Python and Node.js

The interpreted languages are shown for scale, not as primary references. Their runtimes are
3.20x-107.17x and 1.52x-16.77x the C median respectively over the
executed workloads; they exist in the table to demonstrate the size of the gap a compiled
language removes, and to check that every implementation computes the same checksum line.

## Fairness notes and caveats

1. **Idiomatic, not identical, programs.** The suite measures each language as its users
   would write it, and that includes representation choices:

   | workload | lsaca | C | Rust | Python | Node.js |
   | --- | --- | --- | --- | --- | --- |
   | `string_build` | `String::new()` + `format!` then `push_str(&piece)` | one `malloc`ed char buffer of exact size | `String::with_capacity` + `to_string` + `push` | list of `str` + `",".join` | array + `join` |
   | `vec_memory` | `Vec<Point>` + `with_capacity` + `push` | `malloc`ed array of structs | `Vec<Point>` with `with_capacity` | list of tuples | array of objects |
   | `sort_i64` | iterative quicksort on `Vec<i64>` | same quicksort on a `malloc`ed array | `Vec<i64>` + `swap` | list + swapping | array + swapping |
   | `binary_trees` | `Option<Box<Node>>` recursive struct; the by-value traversal clones child subtrees with `.clone()` (moving a field out is `E0507`) | `malloc`ed nodes + raw pointers | `Option<Box<Node>>` if let / match | class with `__slots__` | plain objects |
   | `binary_trees_refs` | same tree, summed via `&Option<Box<Node>>` (no copy) | same program as `binary_trees` (C uses pointers anyway) | same program as `binary_trees` (moving a `Box` is a pointer move) | same program | same program |
   | `sieve` | `vec![0; limit + 1]` of `i64` | `calloc`ed `int64_t` array | `vec![0i64; n]` | list of ints | `Float64Array` |
   | `iterator_pipeline` | `values.iter().filter(...).map(...).fold(...)` lazy adapter structs from the prelude | eager `for` loop with the same arithmetic | its own iterator chain | generator expression inside `sum` | eager loop with `BigInt` squaring |
   | `hashmap_wordcount` | builtin `HashMap<String, i64>` with `get`/`insert`/`keys` | hand-written open-addressing map with `malloc`ed `char *` keys | `std::collections::HashMap` + `entry` | `dict` | `Map` |
   | `generic_stack` | one generic `Stack<T>`, monomorphized twice | two hand-written typed stack implementations | one generic `Stack<T>` | one class `Stack` | one class `Stack` |
   | `fn_pipeline` | `Rule { label: String, apply: fn(i64) -> i64 }` + `.clone()` of a rule | struct with `const char *` and a function pointer | struct with `String` and `fn(i64) -> i64` | class holding a lambda | object holding a function |
   | `string_unicode` | `String` built one `char` at a time; `.chars()`, `char_at`, `to_uppercase`, boundary-checked `substring` | `char *` buffer with byte-oriented UTF-8 handling | `String` with `char` iteration and `to_uppercase` | code-point list + `str.upper` | code-point array + `toUpperCase` |
   | `hashmap_i64` | builtin `HashMap<i64, i64>` with `insert`/`get`/`remove` | hand-written open-addressing map with `int64_t` keys | `std::collections::HashMap<i64, i64>` | `dict` | `Map` |
   | `hashmap_string` | builtin `HashMap<String, i64>` with owned keys | hand-written open-addressing map with `strdup`ed keys | `std::collections::HashMap<String, i64>` | `dict` | `Map` |

   These are all reasonable code, but they do different amounts of low-level work, and
   that is exactly what the benchmarks are measuring.

The following things are deliberately kept visible rather than smoothed over:

- Every number in a table is from one run on one host. Table cells are never combined from different machines, and no cross-machine comparison is made anywhere on this page.
- The runtime tables show the median of 7 measured runs after one warm-up, with the minimum in parentheses. The minimum is there so that a noisy median can be spotted.
- This 2.2.0 run was recorded on a host that was about 12% slower than the host of the 2.1.0 run: the mean lsaca runtime moved +12.2% and the median +13.9%, and the C reference builds moved +15.6% at the median (gcc needed 0.25-0.44 s per program here versus 0.22-0.47 s then). lsaca and C are always measured in the same run, so ratios stay meaningful; deltas between the two published runs mix code changes with host variance.
- Several short workloads moved by tens of percent in both directions. Per-process fixed cost is a large share of any run under about 25 ms, so read those deltas as measured on this host, not as isolated improvements or regressions in generated code.
- `string_build` is still the largest ordinary-workload gap: 108.4 ms, 6.83x C. The 2.1 loss of the format-into-push fusion stands: the idiomatic program binds `let piece = format!("{},", v);` and then calls `s.push_str(&piece);`, so each token allocates and copies a temporary `String`. The ratio improved from 7.48x here only because the C build moved more (+22.1%) than lsaca (+11.5%); the missing optimisation remains a follow-up.
- `sort_i64` improved from 1.71x to 1.41x (56.6 -> 43.6 ms, -23.0%) while its C build moved -6.6%. The 2.1.0 page listed this as a code-generation follow-up after the index casts to `usize`; this run measures the follow-up as largely recovered.
- `string_split` improved from 2.75x to 2.00x (39.3 -> 25.9 ms, -34.2%) with its C build at -9.5%; this is the clearest language-level improvement in the run.
- `binary_trees` intentionally performs by-value tree copies; `binary_trees_refs` is the reference-traversal variant and the one to read when comparing traversal overhead. The borrowed traversal is 36.8 ms (1.77x C) in this run; its C build moved -20.6%, which is why the ratio worsened even though the lsaca time moved -3.6%.
- The `string_unicode` ratio worsened from 2.44x to 2.88x mostly because the C build moved -4.7% on this host while the lsaca time moved +12.3%.
- The `hashmap_string` workload is the one case where the lsaca build was faster than C: 227.5 ms versus 246.2 ms (0.92x).
- The 2.2.0 language additions (arrays, loop labels, reference fields, `From` with `?`, borrowed accessors) did not change the runtime representation: values are still ORC handles, `String` is still guaranteed-valid UTF-8, generics are still monomorphized and the `HashMap` is the same Swiss-table implementation. That is why the shape of the runtime table is stable across 2.1.0 and 2.2.0.
- The four alternative-implementation workloads (`fib_iter`, `fib_memo`, `sort_insertion`, `array_wordcount`) are built in lsaca and C only. Rust, Python and Node.js cells are `n/a` for them by design, not because they failed.
- Compile-only workloads are built but never executed, so they appear only in the compile and artifact tables.
- The cached-build column reuses the cache produced by the cold build of the same program in the same run; it is the best case for incremental builds, not an average edit-rebuild cost.
- Peak memory is the operating system's peak working set for the child process (sampled by the harness), not a per-allocation count. A program that leaks does not necessarily show a high peak in a short run, which is why every lsaca program is also built and run with the debugging runtime that reports leaks.
- Expected checksums are computed by `scripts/oracles.py` independently of lsaca and of the other languages; a mismatch aborts the run instead of publishing it.
- The measurement environment is Windows 11 on an Intel i7-1260P. Absolute numbers will differ on other hardware; the ratios to C are the portable part, and the CI matrix re-runs the quick suite on Linux, macOS and Windows to keep the smoke signal honest.

## Reproducing

Build the compiler, then run the harness on this machine:

```sh
cargo build --release -p lsaca-cli
python scripts/bench.py --compiler target/release/lsaca.exe --build-mode official
```

That writes `benchmarks/results/latest.json` and `benchmarks/results/latest.md`, which are the
sources of every table on this page. Useful variants:

```sh
python scripts/bench.py --quick                       # shorter smoke run
python scripts/bench.py --programs fib,loop_sum       # subset of workloads
python scripts/bench.py --languages lsaca,c           # subset of languages
python scripts/bench.py --scale small,large           # only the smaller/larger data scales
python scripts/bench.py --list-scales                 # sizes and expected output per scale
python scripts/bench_checks.py                        # functional and property checks
python scripts/check_bench21.py                       # verify this page against results
```

To regenerate the derived report sections used on this page:

```sh
python scripts/bench_report.py \
  --baseline benchmarks/results/archive/v2.1/latest.json
```

The harness requires gcc, rustc, node and Python on the PATH and, on Windows, the MSYS2 UCRT64
toolchain directory. It records the compiler version, compiler SHA-256 and repository commit in
the results file so that a published table can always be traced back to the exact binary that
produced it.

3 data scales (small, medium, large) are measured for the argument-driven workloads. The
generated-C bytes in the artifact table are the sizes of the C files the compiler emitted, which
is the input to the C compiler rather than a measure of the lsaca source.

## See also

- [benchmarks/README.md](../../benchmarks/README.md) - harness usage, output layout, how
  to add a workload.
- [benchmarks/results/latest.json](../../benchmarks/results/latest.json) - the raw data
  behind this page.
- [lsaca documentation](../README.md) - the manual hub and full index.
- [Getting started](01-getting-started.md) - the first chapter of the language tour.
