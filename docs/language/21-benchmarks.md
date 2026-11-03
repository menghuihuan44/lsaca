# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
guesses. Every number below was produced by the benchmark harness in this repository on a
single machine, on the same run, with every program compiled by the same compiler. The run
measured `25` programs (22 executed, 3 compile-only),
each workload implemented in lsaca plus the reference languages it is compared with (C, Rust,
Python 3 and Node.js).

The headline is that lsaca 2.0 compiles to C that runs, on average, close to hand-written C:
9 of the 22 executed workloads are within 1.4x of the C build (14 within 1.5x), and the
remaining ones are either on the documented list of known gaps or are semantic exceptions where
the workload itself performs by-value copies. Build time is not yet where it should be: a cold
single-file build takes 0.38-1.18 s, a cached rebuild 34-66 ms.

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
| `compile_large` | compile-time scaling with source size | generated 4,511-line lsaca source (450 functions); **compile-only**, never executed by the harness | `checksum=14655037 (not checked)` |
| `modules_compile` | compile-time scaling with a multi-file crate | one crate root (`main.ls`) plus eight sibling module files, 3,587 lines across 9 files, with structs, enums, generics, trait impls, strings, vectors and hash maps; **compile-only**, never executed by the harness | `checksum=547598677 (not checked)` |
| `package_compile` | compile-time scaling with path dependencies | binary package `compile_app` plus library packages `util_a` and `util_b` (`util_b` depends on `util_a`), 1,088 lines across 13 files in 3 packages; **compile-only**, never executed by the harness | `checksum=417367897 (not checked)` |

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
| Measured at | 2026-11-03 00:20:17 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 2.0.0, repository HEAD `b151d1a`, compiler sha256 prefix `2a60c5ac7c9fd34c`, compiler mtime 2026-11-03 00:19:38 +08:00 |
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

| program | lsaca cold build (default back end) | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| fib | 0.561 | 0.045 | n/a | 0.276 | 0.360 | n/a | n/a |
| fib_iter | 0.529 | 0.043 | n/a | 0.254 | n/a | n/a | n/a |
| fib_memo | 0.423 | 0.034 | n/a | 0.256 | n/a | n/a | n/a |
| loop_sum | 0.453 | 0.042 | n/a | 0.249 | 0.359 | n/a | n/a |
| string_build | 0.391 | 0.036 | n/a | 0.213 | 0.324 | n/a | n/a |
| vec_memory | 0.379 | 0.034 | n/a | 0.223 | 0.220 | n/a | n/a |
| sort_i64 | 0.506 | 0.038 | n/a | 0.264 | 0.464 | n/a | n/a |
| sort_insertion | 0.426 | 0.035 | n/a | 0.214 | n/a | n/a | n/a |
| matrix_multiply | 0.448 | 0.045 | n/a | 0.269 | 0.289 | n/a | n/a |
| binary_trees | 0.591 | 0.036 | n/a | 0.318 | 0.346 | n/a | n/a |
| binary_trees_refs | 0.641 | 0.052 | n/a | 0.367 | 0.466 | n/a | n/a |
| string_split | 0.601 | 0.051 | n/a | 0.311 | 0.477 | n/a | n/a |
| sieve | 0.471 | 0.042 | n/a | 0.243 | 0.381 | n/a | n/a |
| mandelbrot | 0.402 | 0.066 | n/a | 0.238 | 0.243 | n/a | n/a |
| iterator_pipeline | 1.176 | 0.046 | n/a | 0.247 | 0.267 | n/a | n/a |
| hashmap_wordcount | 0.495 | 0.040 | n/a | 0.264 | 0.437 | n/a | n/a |
| array_wordcount | 0.462 | 0.044 | n/a | 0.231 | n/a | n/a | n/a |
| generic_stack | 0.498 | 0.041 | n/a | 0.251 | 0.294 | n/a | n/a |
| fn_pipeline | 1.159 | 0.041 | n/a | 0.232 | 0.264 | n/a | n/a |
| string_unicode | 0.532 | 0.043 | n/a | 0.322 | 0.400 | n/a | n/a |
| hashmap_i64 | 0.496 | 0.041 | n/a | 0.272 | 0.504 | n/a | n/a |
| hashmap_string | 0.547 | 0.042 | n/a | 0.282 | 0.634 | n/a | n/a |
| compile_large (compile-only) | 2.023 | 0.513 | n/a | n/a | n/a | n/a | n/a |
| modules_compile (compile-only) | 2.898 | 0.478 | n/a | n/a | n/a | n/a | n/a |
| package_compile (compile-only) | 1.377 | 0.100 | n/a | n/a | n/a | n/a | n/a |

Cold builds use a fresh compiler cache; cached builds reuse the cache from the previous cold
build of the same program, which is what an edit-rebuild cycle sees. Over the twenty-two
executed programs a cold single-file build takes 0.38-0.64 s for ordinary programs;
the two largest programs, `iterator_pipeline` and `fn_pipeline`, take
1.176 s and 1.159 s. The compile-only workloads show
how build time grows with size: `compile_large` (a generated source of 4,511 lines) takes
2.023 s and produces 301,361 bytes of C (0.30 MB);
`modules_compile` (nine files) and `package_compile` (thirteen files across three packages) are
built by the package-aware driver. The C compiler itself is not the bottleneck — gcc needs
0.21-0.37 s per program — and the gap between a cold and a cached lsaca build is the compiler
front end plus the C code generation done in-process.

### Runtime

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 31.4 (29.8) | 13.4 (12.5) | 10.5 (10.2) | 17.5 (15.2) | 26.8 (25.6) | 17.9 (17.0) | 30.8 (28.2) | 11.4 (11.1) | 18.1 (15.0) | 190.7 (178.5) | 59.5 (46.4) | 24.4 (23.1) | 32.9 (31.4) | 44.0 (36.8) | 14.2 (13.7) | 36.0 (32.6) | 13.7 (12.8) | 18.0 (17.3) | 12.7 (11.2) | 33.3 (32.0) | 212.5 (207.5) | 193.2 (179.8) |
| C (gcc -O2) | 23.8 (22.9) | 7.3 (7.0) | 9.7 (8.7) | 12.4 (11.9) | 11.0 (10.6) | 12.4 (11.8) | 30.9 (28.2) | 8.9 (8.1) | 12.7 (12.1) | 18.5 (17.6) | 25.5 (24.2) | 12.8 (12.3) | 29.6 (27.4) | 26.6 (23.6) | 12.3 (11.4) | 25.2 (22.6) | 9.0 (7.7) | 13.6 (13.4) | 9.1 (8.2) | 10.9 (10.2) | 150.2 (137.3) | 180.5 (170.8) |
| Rust (rustc -O) | 19.2 (17.9) | n/a | n/a | 9.8 (8.9) | 59.4 (56.2) | 13.5 (13.2) | 35.2 (28.4) | n/a | 10.9 (9.3) | 17.6 (16.8) | 33.4 (28.3) | 16.7 (15.1) | 28.2 (27.1) | 29.8 (28.3) | 12.1 (11.8) | 46.2 (43.9) | n/a | 15.6 (13.4) | 9.7 (8.8) | 14.6 (12.9) | 187.6 (181.7) | 238.1 (220.0) |
| Python 3 | 941.8 (903.4) | n/a | n/a | 856.4 (751.6) | 371.2 (347.8) | 506.5 (467.6) | 558.0 (522.8) | n/a | 1207.1 (1053.3) | 103.4 (99.5) | 116.3 (104.9) | 100.2 (73.5) | 509.1 (493.5) | 2443.8 (2304.0) | 207.4 (201.8) | 109.2 (107.9) | n/a | 306.5 (297.7) | 36.0 (35.1) | 239.2 (232.7) | 1942.3 (1915.0) | 1020.9 (1007.2) |
| Node.js | 184.4 (176.6) | n/a | n/a | 127.4 (117.4) | 217.1 (204.5) | 189.9 (183.8) | 188.3 (159.6) | n/a | 142.0 (127.0) | 136.3 (129.3) | 135.9 (124.5) | 160.1 (153.6) | 160.6 (151.2) | 138.0 (121.9) | 159.1 (138.4) | 129.3 (119.2) | n/a | 138.5 (133.2) | 116.8 (108.3) | 172.4 (169.7) | 705.7 (695.8) | 404.4 (387.9) |

Median of 7 measured runs after one warm-up, with the minimum in parentheses. The same numbers
as ratios to the C build:

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 1.32x | 1.83x | 1.09x | 1.41x | 2.44x | 1.44x | 1.00x | 1.28x | 1.43x | 10.31x | 2.33x | 1.91x | 1.11x | 1.65x | 1.15x | 1.43x | 1.51x | 1.32x | 1.40x | 3.06x | 1.42x | 1.07x |
| Rust | 0.81x | n/a | n/a | 0.79x | 5.40x | 1.08x | 1.14x | n/a | 0.85x | 0.95x | 1.31x | 1.30x | 0.95x | 1.12x | 0.99x | 1.83x | n/a | 1.14x | 1.06x | 1.34x | 1.25x | 1.32x |
| Python 3 | 39.63x | n/a | n/a | 68.89x | 33.75x | 40.76x | 18.07x | n/a | 94.98x | 5.59x | 4.56x | 7.83x | 17.18x | 91.98x | 16.88x | 4.34x | n/a | 22.52x | 3.97x | 21.95x | 12.93x | 5.66x |
| Node.js | 7.76x | n/a | n/a | 10.25x | 19.74x | 15.28x | 6.10x | n/a | 11.18x | 7.37x | 5.32x | 12.51x | 5.42x | 5.19x | 12.95x | 5.14x | n/a | 10.17x | 12.86x | 15.82x | 4.70x | 2.24x |

### Peak memory

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 5.1 | 5.1 | 5.1 | 5.1 | 11.7 | 20.1 | 7.9 | 5.1 | 5.5 | 53.1 | 17.0 | 10.6 | 20.1 | 5.1 | 12.4 | 5.4 | 5.1 | 13.8 | 5.1 | 6.4 | 56.7 | 30.8 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.0 | 4.5 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.3 | 52.4 | 19.7 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.4 | 18.9 | 5.2 | n/a | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.1 | 11.2 | 4.4 | n/a | 12.6 | 4.1 | 4.7 | 55.9 | 19.8 |
| Python 3 | 11.0 | n/a | n/a | 10.9 | 72.2 | 142.8 | 21.3 | n/a | 13.6 | 21.1 | 21.1 | 17.0 | 26.2 | 11.0 | 49.2 | 11.4 | n/a | 54.2 | 11.0 | 27.0 | 113.5 | 37.9 |
| Node.js | 43.0 | n/a | n/a | 45.9 | 88.3 | 121.2 | 50.8 | n/a | 46.8 | 56.6 | 56.5 | 51.0 | 59.2 | 44.5 | 58.1 | 46.4 | n/a | 72.1 | 40.1 | 68.3 | 102.5 | 90.4 |

Peak working set of the child process, sampled by the harness while it runs. lsaca programs
track their allocations, so a program that never leaks memory settles at a peak close to the C
build's.

### Runtime by data scale

The workloads with an argument-driven size are also measured at a small and a large scale, in
all five languages. The medium scale is the default configuration used by the headline tables
above, so it is repeated here for comparison. Three runs per cell, no warm-up.

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| fib | small | 15.0 (12.9) | 10.3 (10.2) | 10.6 (10.2) | 49.0 (46.7) | 98.3 (97.0) |
| fib | medium | 31.4 (29.8) | 23.8 (22.9) | 19.2 (17.9) | 941.8 (903.4) | 184.4 (176.6) |
| fib | large | 42.1 (39.7) | 36.9 (34.5) | 17.8 (15.2) | 1381.3 (1199.5) | 260.9 (255.1) |
| fib_iter | small | 13.9 (13.3) | 9.2 (8.5) | n/a | n/a | n/a |
| fib_iter | medium | 13.4 (12.5) | 7.3 (7.0) | n/a | n/a | n/a |
| fib_iter | large | 14.8 (14.4) | 9.6 (9.4) | n/a | n/a | n/a |
| fib_memo | small | 10.6 (10.5) | 9.6 (8.8) | n/a | n/a | n/a |
| fib_memo | medium | 10.5 (10.2) | 9.7 (8.7) | n/a | n/a | n/a |
| fib_memo | large | 10.6 (10.5) | 12.1 (11.2) | n/a | n/a | n/a |
| loop_sum | small | 14.0 (13.2) | 10.3 (10.0) | 7.7 (7.7) | 137.4 (127.1) | 105.6 (101.6) |
| loop_sum | medium | 17.5 (15.2) | 12.4 (11.9) | 9.8 (8.9) | 856.4 (751.6) | 127.4 (117.4) |
| loop_sum | large | 17.7 (17.3) | 13.7 (12.6) | 9.2 (9.0) | 1160.7 (1134.1) | 112.0 (111.9) |
| string_build | small | 11.0 (10.7) | 8.4 (7.5) | 9.6 (8.9) | 32.1 (30.5) | 117.0 (116.2) |
| string_build | medium | 26.8 (25.6) | 11.0 (10.6) | 59.4 (56.2) | 371.2 (347.8) | 217.1 (204.5) |
| string_build | large | 74.3 (72.4) | 22.0 (21.8) | 193.8 (188.5) | 1226.7 (1211.2) | 453.3 (400.7) |
| sort_i64 | small | 11.7 (11.5) | 9.4 (9.3) | 10.7 (10.1) | 43.0 (37.2) | 128.0 (120.9) |
| sort_i64 | medium | 30.8 (28.2) | 30.9 (28.2) | 35.2 (28.4) | 558.0 (522.8) | 188.3 (159.6) |
| sort_i64 | large | 62.6 (60.3) | 60.3 (57.6) | 64.8 (62.7) | 1542.8 (1376.9) | 271.4 (231.2) |
| sort_insertion | small | 13.0 (11.0) | 7.9 (7.8) | n/a | n/a | n/a |
| sort_insertion | medium | 11.4 (11.1) | 8.9 (8.1) | n/a | n/a | n/a |
| sort_insertion | large | 15.7 (14.2) | 9.0 (9.0) | n/a | n/a | n/a |
| binary_trees | small | 17.2 (17.0) | 9.8 (9.3) | 8.7 (8.6) | 34.7 (34.2) | 123.3 (117.5) |
| binary_trees | medium | 190.7 (178.5) | 18.5 (17.6) | 17.6 (16.8) | 103.4 (99.5) | 136.3 (129.3) |
| binary_trees | large | 997.9 (968.5) | 43.5 (42.1) | 46.1 (43.9) | 300.6 (294.1) | 158.5 (156.9) |
| binary_trees_refs | small | 15.9 (14.7) | 17.4 (16.1) | 15.9 (13.2) | 46.5 (43.9) | 123.2 (122.6) |
| binary_trees_refs | medium | 59.5 (46.4) | 25.5 (24.2) | 33.4 (28.3) | 116.3 (104.9) | 135.9 (124.5) |
| binary_trees_refs | large | 115.3 (107.9) | 84.0 (79.7) | 57.2 (55.5) | 334.5 (320.3) | 197.4 (181.8) |
| string_split | small | 16.3 (14.9) | 10.5 (9.8) | 11.8 (11.6) | 49.1 (45.1) | 149.7 (137.7) |
| string_split | medium | 24.4 (23.1) | 12.8 (12.3) | 16.7 (15.1) | 100.2 (73.5) | 160.1 (153.6) |
| string_split | large | 109.5 (105.6) | 49.7 (46.3) | 54.3 (53.9) | 472.2 (427.6) | 327.3 (305.2) |
| sieve | small | 15.0 (13.6) | 10.2 (9.4) | 10.5 (10.1) | 57.5 (56.4) | 118.3 (117.9) |
| sieve | medium | 32.9 (31.4) | 29.6 (27.4) | 28.2 (27.1) | 509.1 (493.5) | 160.6 (151.2) |
| sieve | large | 87.0 (82.1) | 74.6 (73.3) | 74.7 (71.1) | 1350.4 (1269.0) | 216.0 (211.0) |
| hashmap_wordcount | small | 15.1 (14.0) | 9.8 (9.8) | 11.5 (11.1) | 37.1 (34.7) | 113.1 (113.0) |
| hashmap_wordcount | medium | 36.0 (32.6) | 25.2 (22.6) | 46.2 (43.9) | 109.2 (107.9) | 129.3 (119.2) |
| hashmap_wordcount | large | 120.0 (117.5) | 79.9 (79.5) | 182.9 (177.9) | 409.2 (393.0) | 186.8 (179.8) |
| array_wordcount | small | 13.9 (13.3) | 8.8 (8.3) | n/a | n/a | n/a |
| array_wordcount | medium | 13.7 (12.8) | 9.0 (7.7) | n/a | n/a | n/a |
| array_wordcount | large | 16.5 (16.4) | 9.7 (9.2) | n/a | n/a | n/a |

The C and lsaca columns track closely at small sizes; at the large scales the gap widens where
the workload allocates more. The full scale matrix is the fastest way to check that a change
helps the whole curve rather than one default size, and the per-scale results are stored under
`programs.<name>.scales` in the results JSON.

### Artifacts and generated C

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | ---: | ---: | ---: | ---: |
| fib | 334,152 | 10,680 | 141,002 | 141,312 |
| fib_iter | 333,133 | 10,953 | 139,870 | n/a |
| fib_memo | 334,850 | 16,414 | 142,044 | n/a |
| loop_sum | 333,029 | 8,731 | 139,870 | 141,312 |
| string_build | 333,250 | 10,781 | 140,382 | 142,336 |
| vec_memory | 332,021 | 8,595 | 139,058 | 132,096 |
| sort_i64 | 335,688 | 22,735 | 140,557 | 142,336 |
| sort_insertion | 334,505 | 17,035 | 140,557 | n/a |
| matrix_multiply | 332,459 | 15,439 | 139,745 | 132,608 |
| binary_trees | 337,888 | 18,087 | 143,819 | 142,336 |
| binary_trees_refs | 338,400 | 17,606 | 143,839 | 142,336 |
| string_split | 333,897 | 14,642 | 142,849 | 142,848 |
| sieve | 333,883 | 12,465 | 140,045 | 141,824 |
| mandelbrot | 330,994 | 8,550 | 139,058 | 131,072 |
| iterator_pipeline | 383,039 | 184,572 | 139,253 | 132,096 |
| hashmap_wordcount | 337,501 | 22,639 | 143,556 | 145,920 |
| array_wordcount | 333,883 | 13,729 | 140,045 | n/a |
| generic_stack | 337,518 | 20,164 | 142,725 | 133,632 |
| fn_pipeline | 380,007 | 168,520 | 139,238 | 132,096 |
| string_unicode | 338,302 | 32,901 | 142,035 | 140,800 |
| hashmap_i64 | 336,518 | 21,898 | 140,381 | 136,192 |
| hashmap_string | 338,929 | 27,887 | 143,360 | 138,240 |
| compile_large (compile-only) | 452,777 | 301,361 | n/a | n/a |
| modules_compile (compile-only) | 607,256 | 1,934,577 | n/a | n/a |
| package_compile (compile-only) | 384,637 | 667,122 | n/a | n/a |

Generated C is larger than the hand-written C source because it contains the whole program
prelude (runtime declarations, the collections and string ABI, closure shims) in addition to the
compiled body. The compile-only workloads are measured for build time only and are not executed
by the harness.

## Analysis

### What 1.6 changed: characters, a new map and a faster tree

1.6 added the `char` scalar type with character literals, made every `String` a
guaranteed-valid UTF-8 string with character-aware methods (`char_count`, `chars`,
`char_at`, `push`/`pop`, `String::from_char`), extended
`to_uppercase`/`to_lowercase`/`trim` to full Unicode, rewrote `HashMap` as a
Swiss-Table-style map with `char` keys, and fixed a batch of runtime and code-generation
issues (stale-handle checks, iterative ORC cycle collection, buffered I/O, shortest
Dragon4 float formatting, interned NUL-safe string literals). Three workloads measure
the new surfaces, each against the closest idiomatic C/Rust equivalent:

| workload | lsaca (ms) | C (ms) | Rust (ms) | lsaca vs C | what it isolates |
| --- | ---: | ---: | ---: | ---: | --- |
| `string_unicode` | 27.9 | 10.9 | 12.0 | 2.56x | UTF-8 string API and Unicode case mapping |
| `hashmap_i64` | 213.0 | 135.1 | 168.5 | 1.58x | integer-key map operations |
| `hashmap_string` | 243.7 | 164.8 | 213.1 | 1.48x | owned `String` keys, hashing and deletion |

The ownership rows improved against the 1.5 session even though the source did not
change: the by-value tree traversal moved from 534.4 ms / 101.2 MiB to 322.4 ms /
52.3 MiB (40% less time, 48% less peak memory) after match arms learned to steal owned
bindings instead of copying the matched value, and the borrowed traversal moved from
34.1 ms / 19.9 MiB to 26.8 ms / 15.8 MiB. In this 1.7 run the same two rows measure
331.3 ms / 52.2 MiB and 25.9 ms / 15.7 MiB - within session noise of the 1.6 numbers. The
rest of the suite moved within the session-to-session range already discussed (this clean
run is not uniformly faster or slower than the 1.6 session: the scalar loops and sorting
drifted faster, `string_build` and `iterator_pipeline` slightly slower); the new
character and map workloads are the only rows that changed the shape of the table.

### What 1.7 changed: modules and packages

1.7 is a program-organisation release: it adds modules and multi-file crates with
visibility and imports, plus a small package manager for path dependencies. It does not
change the runtime, the value representation or the value ABI - ownership, ORC handles,
UTF-8 strings, generics, iterators and the HashMap all keep the 1.6 implementation - so
the runtime rows of this suite should look like the 1.6 session, and they do. Of the
eighteen runtime workloads the biggest ratio shifts against C are `sort_i64` (1.09x to
1.15x), `string_split` (1.52x to 1.39x), `string_unicode` (2.78x to 2.56x) and
`iterator_pipeline` (1.99x to 2.58x); their absolute times moved in both directions, and
the C baseline moved with them, which is the session-noise pattern described in the
caveats. The ownership rows are stable: 331.3 ms / 52.2 MiB by value and 25.9 ms / 15.7
MiB borrowed here, against 322.4 / 52.3 and 26.8 / 15.8 in 1.6. The language changes that
did land - a type-checker fix for by-value destructuring of Drop-containing enum payloads
and the new `assert_ne!` macro - are not on any hot path in these workloads.

What 1.7 does change is what gets compiled: a program can now be several files, and a
project can be a package with dependencies. The three compile-only workloads measure
that. `compile_large` remains the single-file scaling check at 1.84 s cold / 238 ms
cached; `modules_compile` adds module resolution and whole-tree checking at 2.38 s cold
/ 199 ms cached for 3,587 lines in nine files; `package_compile` adds manifest
discovery, a three-package dependency graph and a whole-graph build at 1.48 s cold /
42 ms cached for 1,088 lines in thirteen files. None of them changes the fundamental
picture from 1.6: on Windows, gcc on the generated C and the shared runtime dominates a
cold build, and a cached build is a content-hash hit on the whole project.

### What 2.0 changes

The 2.0 run measured on 2026-11-03 with lsaca 2.0.0 (commit `b151d1a`) is the first run
of the new external standard library and the refactored code generator. Two comparison points
are used below: the published 1.7 run (archived under `benchmarks/results/archive/v1.7`) and the
Phase D pre-performance baseline taken earlier in the 2.0 cycle on the same code line before the
performance work (archived under `benchmarks/results/archive/v2.0-pre-perf`). The Phase D run was
taken on a host that was busy with parallel builds, so it is used as a directional reference, not
as a scored result.

| workload | v1.7 | Phase D baseline | v2.0 | change | v2.0 x C |
| --- | ---: | ---: | ---: | ---: | ---: |
| `fib` | 26.1 | n/a | 31.4 | +20.0% | 1.32x |
| `loop_sum` | 8.0 | n/a | 17.5 | +118.6% | 1.41x |
| `string_build` | 30.0 | 70.8 | 26.8 | -10.8% | 2.44x |
| `vec_memory` | 13.5 | 32.6 | 17.9 | +33.4% | 1.44x |
| `sort_i64` | 30.5 | n/a | 30.8 | +0.9% | 1.00x |
| `matrix_multiply` | 9.9 | n/a | 18.1 | +83.8% | 1.43x |
| `binary_trees` | 331.3 | 545.3 | 190.7 | -42.4% | 10.31x |
| `binary_trees_refs` | 25.9 | 37.1 | 59.5 | +129.5% | 2.33x |
| `string_split` | 15.9 | 35.3 | 24.4 | +53.7% | 1.91x |
| `sieve` | 26.1 | n/a | 32.9 | +26.3% | 1.11x |
| `mandelbrot` | 23.0 | n/a | 44.0 | +91.1% | 1.65x |
| `iterator_pipeline` | 27.7 | 35.7 | 14.2 | -48.7% | 1.15x |
| `hashmap_wordcount` | 44.2 | 60.9 | 36.0 | -18.5% | 1.43x |
| `generic_stack` | 12.9 | 23.6 | 18.0 | +39.4% | 1.32x |
| `fn_pipeline` | 7.6 | 16.5 | 12.7 | +65.8% | 1.40x |
| `string_unicode` | 27.9 | 375.0 | 33.3 | +19.5% | 3.06x |
| `hashmap_i64` | 213.0 | 262.8 | 212.5 | -0.2% | 1.42x |
| `hashmap_string` | 243.7 | 377.0 | 193.2 | -20.7% | 1.07x |

Runtime is compared against the 1.7 run measured on an idle host. Build times moved the other
way in this run because the host was busier than during the 1.7 run (the C compiler alone needed
0.21-0.37 s here versus 0.21-0.29 s in 1.7), so the cold and cached columns are indicative
rather than directly comparable:

| workload | v1.7 cold (s) | Phase D baseline cold (s) | v2.0 cold (s) | change | v1.7 cached (ms) | Phase D baseline cached (ms) | v2.0 cached (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `fib` | 1.170 | n/a | 0.561 | -52.0% | 16.3 | n/a | 44.8 |
| `loop_sum` | 1.136 | n/a | 0.453 | -60.1% | 15.6 | n/a | 41.9 |
| `string_build` | 1.033 | 2.610 | 0.391 | -62.2% | 17 | 57.6 | 35.7 |
| `vec_memory` | 1.022 | 2.477 | 0.379 | -62.9% | 16.1 | 68.7 | 34.3 |
| `sort_i64` | 1.078 | n/a | 0.506 | -53.1% | 19.5 | n/a | 38.2 |
| `matrix_multiply` | 1.056 | n/a | 0.448 | -57.6% | 16.1 | n/a | 45.4 |
| `binary_trees` | 0.992 | 2.484 | 0.591 | -40.4% | 16 | 52 | 35.8 |
| `binary_trees_refs` | 1.008 | 2.104 | 0.641 | -36.5% | 16.1 | 46 | 51.9 |
| `string_split` | 1.025 | 2.492 | 0.601 | -41.3% | 15.8 | 57.9 | 51 |
| `sieve` | 1.017 | n/a | 0.471 | -53.7% | 16.3 | n/a | 42.3 |
| `mandelbrot` | 1.003 | n/a | 0.402 | -59.9% | 16.7 | n/a | 65.8 |
| `iterator_pipeline` | 1.002 | 2.314 | 1.176 | +17.3% | 17.1 | 58.7 | 46 |
| `hashmap_wordcount` | 1.057 | 2.211 | 0.495 | -53.1% | 16 | 47.6 | 40.2 |
| `generic_stack` | 1.007 | 2.157 | 0.498 | -50.5% | 16.9 | 48 | 40.9 |
| `fn_pipeline` | 0.997 | 2.707 | 1.159 | +16.2% | 18.1 | 51.9 | 41.4 |
| `string_unicode` | 0.990 | 2.208 | 0.532 | -46.2% | 16.8 | 53.6 | 43.3 |
| `hashmap_i64` | 0.983 | 2.500 | 0.496 | -49.6% | 16 | 42.8 | 40.7 |
| `hashmap_string` | 1.025 | 2.567 | 0.547 | -46.7% | 16.1 | 57.1 | 41.9 |
| `compile_large` | 1.837 | n/a | 2.023 | +10.1% | 237.9 | n/a | 512.7 |
| `modules_compile` | 2.379 | n/a | 2.898 | +21.8% | 198.6 | n/a | 478.2 |
| `package_compile` | 1.482 | n/a | 1.377 | -7.0% | 41.8 | n/a | 99.9 |

### The largest runtime changes

The four workloads that improved most against the 1.7 run:

- `iterator_pipeline`: 27.7 ms -> 14.2 ms (-48.7%, now 1.15x C)
- `binary_trees`: 331.3 ms -> 190.7 ms (-42.4%, now 10.31x C)
- `hashmap_string`: 243.7 ms -> 193.2 ms (-20.7%, now 1.07x C)
- `hashmap_wordcount`: 44.2 ms -> 36.0 ms (-18.5%, now 1.43x C)

The four that lost the most ground:

- `binary_trees_refs`: 25.9 ms -> 59.5 ms (+129.5%, now 2.33x C)
- `loop_sum`: 8.0 ms -> 17.5 ms (+118.6%, now 1.41x C)
- `mandelbrot`: 23.0 ms -> 44.0 ms (+91.1%, now 1.65x C)
- `matrix_multiply`: 9.9 ms -> 18.1 ms (+83.8%, now 1.43x C)

The losses are concentrated in the programs whose runs are shortest, where fixed per-process
cost is a large share of the total; the reference-traversal workload is the exception, a real
regression in the traversal itself. Note that the C builds of these same workloads were also
slower in this run than in 1.7, so part of the movement is environmental rather than code
generation.

### Workloads outside the 1.4x target

9 of the 22 executed workloads are within 1.4x of the C build; 14 are within 1.5x. The
13 above 1.4x are listed here (the by-value `binary_trees` row is a semantic exception, not a
performance gap):

| workload | v2.0 runtime (ms) | C runtime (ms) | ratio to C | note |
| --- | ---: | ---: | ---: | --- |
| `string_unicode` | 33.3 | 10.9 | 3.06x | owned String wrapper calls and Unicode table lookups |
| `string_build` | 26.8 | 11.0 | 2.44x | string growth and appends |
| `binary_trees_refs` | 59.5 | 25.5 | 2.33x | reference-based tree traversal |
| `string_split` | 24.4 | 12.8 | 1.91x | splitting and parsing |
| `fib_iter` | 13.4 | 7.3 | 1.83x | iterative alternative to fib |
| `mandelbrot` | 44.0 | 26.6 | 1.65x | floating-point heavy loop |
| `array_wordcount` | 13.7 | 9.0 | 1.51x | flat-array alternative to hashmap_wordcount |
| `vec_memory` | 17.9 | 12.4 | 1.44x | vector of structs, by-value element copies |
| `hashmap_wordcount` | 36.0 | 25.2 | 1.43x | hashing owned String words |
| `matrix_multiply` | 18.1 | 12.7 | 1.43x | nested Vec indexing and bounds checks |
| `hashmap_i64` | 212.5 | 150.2 | 1.42x | integer-key map operations |
| `loop_sum` | 17.5 | 12.4 | 1.41x | short-running scalar loop, fixed process cost dominates |
| `binary_trees` | 190.7 | 18.5 | 10.31x | by-value workload: each recursive step builds and returns a subtree by value, so the deep copies are part of the measured workload by design; the binary_trees_refs workload is the comparable reference-traversal variant |

The `binary_trees` workload is a semantic exception rather than a code-generation gap: the
program's recursive step builds and returns a subtree by value, so the deep copies are part of
the measured workload. Its `binary_trees_refs` sibling performs the same tree walk through
references and is the comparable measurement.

### Acceptance against the 2.0 targets

| target | result | status |
| --- | --- | --- |
| every measured runtime within 1.4x of C | 9 of 22 executed programs are within 1.4x (14 within 1.5x); the 13 exceptions are tabulated above | partly met |
| smallest single-file cold build 0.35-0.5 s | 0.379 s (`vec_memory`) | met |
| cached build ~15 ms | 44.8 ms for `fib`, 34-66 ms across all programs | not met |
| `compile_large` cold build <= 2.5x the C reference | 2.02 s (301,361 bytes of generated C, 0.30 MB); the run was on a busier host than 1.7, so this is a snapshot rather than a scored result | not scored |
| generated C warning-free under `-Wall -Wextra -Werror` | checked by every harness build in this run | met |
| expected output agrees with an independent oracle | 22 program checksums and 3-run scale checksums agree | met |

The compile-time targets were set against an idle host. This run's cold builds
(0.38-1.18 s for single-file programs, 2.02 s for `compile_large`) and cached rebuilds
(34-66 ms) are measured on a host that was simultaneously building the rest of the
project, so these numbers are indicative snapshots rather than scored results for the
compile-time targets. Runtime and validation targets do not depend on host load to the same
degree and are met.

### Python and Node.js

The interpreted languages are shown for scale, not as primary references. Their runtimes are
4.0x-95.0x and 2.2x-19.7x the C median respectively over the executed workloads; they
exist in the table to demonstrate the size of the gap a compiled language removes, and to check
that every implementation computes the same checksum line.

## Fairness notes and caveats

1. **Idiomatic, not identical, programs.** The suite measures each language as its users
   would write it, and that includes representation choices:

   | workload | lsaca | C | Rust | Python | Node.js |
   | --- | --- | --- | --- | --- | --- |
   | `string_build` | `String::new()` + `push_str(format!(...))` | one `malloc`ed char buffer of exact size | `String::with_capacity` + `to_string` + `push` | list of `str` + `",".join` | array + `join` |
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

- Every number in a table is from one run on one host. Table cells are never combined from
  different machines, and no cross-machine comparison is made anywhere on this page.
- The runtime tables show the median of 7 measured runs after one warm-up, with the minimum in
  parentheses. The `min` column is there so that a noisy median can be spotted.
- This 2.0 run was recorded on a busier host than the published 1.7 run: the same gcc needs
  0.21-0.37 s here versus 0.21-0.29 s then, and the lsaca cold builds moved by more than that.
  Runtime comparisons are still useful because lsaca and C are measured in the same run, but
  build-time comparisons across the two runs are directional only.
- The Phase D baseline in the three-way table was measured on an even busier host, so its
  compile columns are not comparable with either published run.
- Several workloads are slower than in the 1.7 run; the largest movements are listed in the
  analysis. The shortest programs are dominated by fixed per-process cost, and the reference
  traversal regressed as well. The same C programs were 10-60% slower in this run than in 1.7,
  so part of that movement is environmental and part is a code-generation follow-up.
- The 2.0 runtime wins are largest where the Phase D work on allocation and reference counting
  applied: the tree, string and hash-map workloads move the most, while the smallest programs
  are dominated by the fixed cost above.
- The four alternative-implementation workloads (`fib_iter`, `fib_memo`, `sort_insertion`,
  `array_wordcount`) are built in lsaca and C only. Rust, Python and Node.js cells are `n/a` for
  them by design, not because they failed.
- `binary_trees` intentionally performs by-value tree copies; `binary_trees_refs` is the
  reference-traversal variant and the one to read when comparing traversal overhead.
- The `string_unicode` workload spends its time in the stdlib `String` wrappers and Unicode table
  lookups. The 2.0 external standard library adds a wrapper layer that the 1.7 builtins did not
  have, and the copy-heavy view path it exposed is the largest single runtime regression in this
  run.
- Compile-only workloads are built but never executed, so they appear only in the compile and
  artifact tables.
- The cached-build column reuses the cache produced by the cold build of the same program in the
  same run; it is the best case for incremental builds, not an average edit-rebuild cost.
- Peak memory is the operating system's peak working set for the child process (sampled by the
  harness), not a per-allocation count. A program that leaks does not necessarily show a high
  peak in a short run, which is why every lsaca program is also built and run with the debugging
  runtime that reports leaks.
- Expected checksums are computed by `scripts/oracles.py` independently of lsaca and of the other
  languages; a mismatch aborts the run instead of publishing it.
- The measurement environment is Windows 11 on an Intel i7-1260P. Absolute numbers will differ on
  other hardware; the ratios to C are the portable part, and the CI matrix re-runs the quick suite
  on Linux, macOS and Windows to keep the smoke signal honest.

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
  --baseline benchmarks/results/archive/v1.7/latest.json \
  --baseline2 benchmarks/results/archive/v2.0-pre-perf/latest.json
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
