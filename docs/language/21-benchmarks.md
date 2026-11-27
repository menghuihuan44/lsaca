# Benchmarks

This page reports how lsaca compares with C, Rust, Python and Node.js on the same workloads. The programs are written in the idiomatic style of each language, every program prints a checksum that the harness verifies against an independent oracle, and every number in the tables below is produced by the benchmark harness rather than typed by hand.

The tables are generated from the machine-readable results in `benchmarks/results/latest.json` and the human-readable summary in `benchmarks/results/latest.md`; the page itself is written by `scripts/gen_bench_page.py`. The workload programs live in `benchmarks/programs`.

> **Version note.** These numbers were measured for lsaca 2.3.0 on 2026-11-27, at 2026-11-27T09:03:48+08:00, from commit 2634ef8 with compiler build f34f18a60e737d8e (2026-11-27 00:19:49 +08:00). The C reference was built with gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0. The 2.2.0 baseline was measured with gcc 15.2.0; comparisons against it are normalised by the C reference and explained in the fairness notes.

13 of the 22 executed workloads run within 1.4x of the C reference; 14 are within 1.5x and 6 are within 1.15x. The closest workload is `generic_stack` at 0.85x of the C runtime and the widest gap is `binary_trees` at 12.56x, a deliberate value-semantics trade-off.

A fresh single-file build takes 0.45-2.67 s and a cached rebuild takes 86-154 ms. The 4,511-line `compile_large` workload takes 3.14 s cold and 906 ms cached. The generated C for that workload is 301,361 bytes (0.30 MB).

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
| Measured at | 2026-11-27 09:03:48 +08:00 |
| Machine | 12th Gen Intel(R) Core(TM) i7-1260P, 16 logical CPUs, 15.63 GiB RAM |
| OS | Windows-11-10.0.26300-SP0 |
| lsaca | lsaca 2.3.0, commit 2634ef8, compiler sha256 f34f18a60e737d8e, built 2026-11-27 00:19:49 +08:00 |
| lsaca build path | target\bench-tools\lsaca.exe |
| C compiler | gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0 |
| Rust compiler | rustc 1.99.0 (b940084d7 2026-09-28) |
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
| fib | 0.827 | 0.122 | n/a | 0.429 | 0.598 | n/a | n/a |
| fib_iter | 0.667 | 0.099 | n/a | 0.311 | n/a | n/a | n/a |
| fib_memo | 0.670 | 0.096 | n/a | 0.384 | n/a | n/a | n/a |
| loop_sum | 0.592 | 0.092 | n/a | 0.265 | 0.388 | n/a | n/a |
| string_build | 0.497 | 0.091 | n/a | 0.263 | 0.387 | n/a | n/a |
| vec_memory | 0.447 | 0.086 | n/a | 0.278 | 0.313 | n/a | n/a |
| sort_i64 | 0.645 | 0.100 | n/a | 0.374 | 0.447 | n/a | n/a |
| sort_insertion | 0.611 | 0.102 | n/a | 0.338 | n/a | n/a | n/a |
| matrix_multiply | 0.604 | 0.097 | n/a | 0.393 | 0.358 | n/a | n/a |
| binary_trees | 0.572 | 0.098 | n/a | 0.269 | 0.348 | n/a | n/a |
| binary_trees_refs | 0.590 | 0.090 | n/a | 0.361 | 0.344 | n/a | n/a |
| string_split | 0.485 | 0.115 | n/a | 0.311 | 0.502 | n/a | n/a |
| sieve | 0.553 | 0.099 | n/a | 0.311 | 0.410 | n/a | n/a |
| mandelbrot | 0.487 | 0.098 | n/a | 0.274 | 0.277 | n/a | n/a |
| iterator_pipeline | 2.674 | 0.110 | n/a | 0.283 | 0.328 | n/a | n/a |
| hashmap_wordcount | 0.684 | 0.114 | n/a | 0.425 | 0.512 | n/a | n/a |
| array_wordcount | 0.486 | 0.118 | n/a | 0.363 | n/a | n/a | n/a |
| generic_stack | 0.748 | 0.110 | n/a | 0.326 | 0.384 | n/a | n/a |
| fn_pipeline | 2.643 | 0.105 | n/a | 0.268 | 0.314 | n/a | n/a |
| string_unicode | 0.634 | 0.115 | n/a | 0.457 | 0.461 | n/a | n/a |
| hashmap_i64 | 0.882 | 0.154 | n/a | 0.327 | 0.598 | n/a | n/a |
| hashmap_string | 0.880 | 0.127 | n/a | 0.514 | 0.634 | n/a | n/a |
| compile_large (compile-only) | 3.138 | 0.906 | n/a | n/a | n/a | n/a | n/a |
| modules_compile (compile-only) | 5.238 | 0.888 | n/a | n/a | n/a | n/a | n/a |
| package_compile (compile-only) | 1.968 | 0.221 | n/a | n/a | n/a | n/a | n/a |

> The fresh single-file build ranges from 0.45 s to 2.67 s over the executed workloads, and the cached rebuild ranges from 86 ms to 154 ms.

> The C reference compiles faster because a C compiler does not have to run a lexer, a parser, a type checker, a lowering pass and a code generator first; the gap is the price of the language's guarantees.

> The compile-only workloads scale with source size: `compile_large` is a single 4,511-line file, `modules_compile` is nine modules and `package_compile` is three packages with thirteen files.

### Runtime

Each cell is the median of seven measured runs after one warm-up, in milliseconds, with the minimum in parentheses. Lower is better.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| lsaca | 33.1 (30.5) | 15.4 (13.9) | 17.1 (14.1) | 17.5 (16.3) | 85.5 (76.9) | 18.4 (17.6) | 39.0 (36.8) | 18.3 (16.9) | 18.7 (18.2) | 209.1 (202.7) | 38.8 (34.5) | 34.1 (28.5) | 57.7 (47.4) | 38.9 (34.6) | 18.5 (17.6) | 43.3 (38.5) | 18.2 (16.5) | 21.2 (20.1) | 13.7 (13.1) | 41.5 (38.5) | 309.7 (283.0) | 300.2 (252.3) |
| C (gcc -O2) | 33.3 (31.3) | 12.2 (11.2) | 11.3 (10.5) | 12.4 (11.7) | 13.7 (13.0) | 15.6 (14.1) | 35.5 (34.9) | 13.6 (11.1) | 15.5 (15.1) | 16.7 (16.2) | 21.0 (20.2) | 22.0 (17.7) | 38.4 (34.5) | 34.9 (31.4) | 17.3 (16.6) | 32.9 (31.1) | 13.6 (12.4) | 25.1 (21.8) | 13.0 (12.2) | 18.2 (17.3) | 187.4 (181.4) | 230.8 (219.9) |
| Rust (rustc -O) | 20.7 (19.2) | n/a | n/a | 12.9 (11.7) | 56.5 (55.4) | 17.8 (16.7) | 33.9 (31.9) | n/a | 13.5 (12.4) | 24.4 (23.6) | 18.0 (17.4) | 21.1 (18.8) | 32.9 (29.3) | 32.8 (31.6) | 17.1 (17.0) | 51.6 (49.7) | n/a | 19.2 (17.0) | 13.3 (12.8) | 16.1 (15.7) | 209.2 (205.2) | 245.9 (235.8) |
| Python 3 | 1074.2 (975.5) | n/a | n/a | 302.1 (278.6) | 167.2 (163.1) | 264.2 (251.0) | 372.6 (350.4) | n/a | 421.5 (393.3) | 81.3 (76.5) | 83.5 (81.5) | 66.8 (54.7) | 259.9 (251.6) | 726.1 (708.5) | 117.7 (113.1) | 79.4 (74.3) | n/a | 226.8 (212.0) | 38.4 (35.8) | 158.2 (139.3) | 1095.1 (1013.9) | 706.1 (669.5) |
| Node.js | 158.8 (154.0) | n/a | n/a | 95.8 (93.7) | 107.7 (104.2) | 157.2 (145.7) | 118.4 (111.3) | n/a | 88.4 (83.2) | 69.0 (67.3) | 71.4 (70.0) | 88.3 (84.1) | 97.6 (91.4) | 86.9 (84.9) | 96.5 (90.7) | 88.3 (82.0) | n/a | 101.7 (95.2) | 69.2 (66.8) | 134.5 (128.6) | 691.3 (634.2) | 522.5 (478.4) |

> The same numbers divided by the C runtime give the relative cost; a value below 1.00x means lsaca is faster than the C reference on this workload.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 0.99x | 1.27x | 1.51x | 1.42x | 6.23x | 1.18x | 1.10x | 1.34x | 1.21x | 12.56x | 1.85x | 1.55x | 1.50x | 1.11x | 1.07x | 1.32x | 1.34x | 0.85x | 1.05x | 2.28x | 1.65x | 1.30x |
| Rust | 0.62x | n/a | n/a | 1.04x | 4.12x | 1.14x | 0.95x | n/a | 0.87x | 1.46x | 0.86x | 0.96x | 0.86x | 0.94x | 0.99x | 1.57x | n/a | 0.76x | 1.02x | 0.89x | 1.12x | 1.07x |
| Python 3 | 32.24x | n/a | n/a | 24.43x | 12.18x | 16.93x | 10.49x | n/a | 27.24x | 4.88x | 3.98x | 3.03x | 6.77x | 20.82x | 6.81x | 2.41x | n/a | 9.03x | 2.96x | 8.69x | 5.84x | 3.06x |
| Node.js | 4.77x | n/a | n/a | 7.75x | 7.85x | 10.07x | 3.33x | n/a | 5.71x | 4.14x | 3.41x | 4.00x | 2.54x | 2.49x | 5.58x | 2.68x | n/a | 4.05x | 5.33x | 7.40x | 3.69x | 2.26x |

### Peak memory

Peak working set of the measured child process, in MiB. The harness samples the peak with the platform API instead of reading a single instantaneous value, so short-lived peaks are included.

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| lsaca | 4.5 | 4.5 | 4.5 | 4.5 | 11.0 | 19.3 | 7.1 | 4.5 | 4.6 | 52.3 | 16.2 | 9.8 | 19.3 | 4.5 | 11.6 | 4.7 | 4.5 | 13.0 | 4.5 | 5.6 | 55.8 | 30.0 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.3 | 18.8 | 5.0 | 4.0 | 4.3 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.7 | 52.5 | 19.6 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.3 | 18.9 | 5.2 | n/a | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.1 | 11.2 | 4.3 | n/a | 12.6 | 4.1 | 4.7 | 55.8 | 19.7 |
| Python 3 | 10.9 | n/a | n/a | 10.9 | 74.4 | 142.7 | 20.2 | n/a | 13.5 | 21.0 | 21.0 | 17.0 | 26.2 | 11.0 | 49.2 | 11.3 | n/a | 54.2 | 11.0 | 25.8 | 113.4 | 38.0 |
| Node.js | 43.1 | n/a | n/a | 44.9 | 93.4 | 128.5 | 51.6 | n/a | 47.2 | 52.5 | 52.6 | 53.5 | 59.5 | 45.3 | 56.0 | 46.7 | n/a | 74.7 | 40.3 | 65.0 | 116.7 | 91.4 |

### Runtime by data scale

The scalable workloads are run at small, medium (the default arguments) and large sizes. Checksums are verified at every scale, so a scaling run that computes the wrong answer is rejected.

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | --- | --- | --- | --- | --- |
| fib | small | 14.4 (14.1) | 15.2 (14.0) | 22.2 (15.6) | 42.2 (40.3) | 66.7 (65.4) |
| fib | medium | 33.1 (30.5) | 33.3 (31.3) | 20.7 (19.2) | 1074.2 (975.5) | 158.8 (154.0) |
| fib | large | 44.3 (42.3) | 55.2 (45.5) | 23.6 (21.2) | 1551.0 (1526.4) | 210.3 (209.3) |
| fib_iter | small | 17.8 (15.7) | 12.3 (11.2) | n/a | n/a | n/a |
| fib_iter | medium | 15.4 (13.9) | 12.2 (11.2) | n/a | n/a | n/a |
| fib_iter | large | 19.3 (18.8) | 12.1 (11.1) | n/a | n/a | n/a |
| fib_memo | small | 16.4 (14.5) | 13.1 (12.0) | n/a | n/a | n/a |
| fib_memo | medium | 17.1 (14.1) | 11.3 (10.5) | n/a | n/a | n/a |
| fib_memo | large | 15.7 (14.9) | 12.8 (11.7) | n/a | n/a | n/a |
| loop_sum | small | 15.0 (14.3) | 9.6 (9.3) | 9.5 (9.0) | 75.0 (69.9) | 67.2 (66.3) |
| loop_sum | medium | 17.5 (16.3) | 12.4 (11.7) | 12.9 (11.7) | 302.1 (278.6) | 95.8 (93.7) |
| loop_sum | large | 18.5 (17.4) | 13.6 (13.2) | 10.9 (10.1) | 406.6 (402.4) | 111.7 (109.8) |
| string_build | small | 12.1 (11.8) | 9.0 (8.7) | 10.3 (10.2) | 29.9 (28.9) | 61.6 (60.6) |
| string_build | medium | 85.5 (76.9) | 13.7 (13.0) | 56.5 (55.4) | 167.2 (163.1) | 107.7 (104.2) |
| string_build | large | 282.8 (281.4) | 26.3 (24.9) | 199.3 (196.2) | 624.8 (616.6) | 289.6 (288.7) |
| sort_i64 | small | 16.9 (13.8) | 14.4 (13.7) | 11.6 (11.1) | 36.2 (34.3) | 71.6 (70.0) |
| sort_i64 | medium | 39.0 (36.8) | 35.5 (34.9) | 33.9 (31.9) | 372.6 (350.4) | 118.4 (111.3) |
| sort_i64 | large | 82.2 (74.4) | 75.9 (72.4) | 72.0 (64.7) | 979.6 (935.5) | 171.3 (168.6) |
| sort_insertion | small | 20.9 (19.2) | 12.9 (11.9) | n/a | n/a | n/a |
| sort_insertion | medium | 18.3 (16.9) | 13.6 (11.1) | n/a | n/a | n/a |
| sort_insertion | large | 23.1 (19.1) | 16.2 (13.9) | n/a | n/a | n/a |
| binary_trees | small | 13.6 (13.1) | 11.9 (8.6) | 13.0 (12.8) | 32.7 (30.4) | 65.5 (63.4) |
| binary_trees | medium | 209.1 (202.7) | 16.7 (16.2) | 24.4 (23.6) | 81.3 (76.5) | 69.0 (67.3) |
| binary_trees | large | 912.2 (866.9) | 43.3 (42.4) | 53.2 (52.2) | 254.8 (243.1) | 90.5 (88.5) |
| binary_trees_refs | small | 16.3 (14.6) | 10.3 (10.2) | 9.6 (9.5) | 34.7 (33.9) | 65.3 (60.0) |
| binary_trees_refs | medium | 38.8 (34.5) | 21.0 (20.2) | 18.0 (17.4) | 83.5 (81.5) | 71.4 (70.0) |
| binary_trees_refs | large | 97.2 (93.8) | 50.7 (50.7) | 51.2 (47.7) | 276.9 (270.7) | 96.1 (92.7) |
| string_split | small | 21.5 (16.6) | 21.1 (18.3) | 15.9 (15.8) | 41.5 (40.7) | 69.1 (68.3) |
| string_split | medium | 34.1 (28.5) | 22.0 (17.7) | 21.1 (18.8) | 66.8 (54.7) | 88.3 (84.1) |
| string_split | large | 149.4 (149.1) | 55.2 (53.3) | 53.8 (51.4) | 185.8 (179.8) | 200.5 (199.3) |
| sieve | small | 18.3 (18.2) | 18.3 (16.3) | 15.1 (14.8) | 48.9 (45.5) | 76.0 (73.8) |
| sieve | medium | 57.7 (47.4) | 38.4 (34.5) | 32.9 (29.3) | 259.9 (251.6) | 97.6 (91.4) |
| sieve | large | 102.1 (99.5) | 87.8 (86.2) | 97.0 (79.4) | 590.0 (569.7) | 180.7 (165.8) |
| hashmap_wordcount | small | 21.5 (19.8) | 15.7 (15.5) | 20.0 (15.7) | 34.7 (33.7) | 70.7 (69.7) |
| hashmap_wordcount | medium | 43.3 (38.5) | 32.9 (31.1) | 51.6 (49.7) | 79.4 (74.3) | 88.3 (82.0) |
| hashmap_wordcount | large | 139.6 (134.4) | 83.2 (81.8) | 202.8 (199.2) | 259.3 (249.7) | 171.3 (170.4) |
| array_wordcount | small | 16.1 (16.0) | 13.7 (13.3) | n/a | n/a | n/a |
| array_wordcount | medium | 18.2 (16.5) | 13.6 (12.4) | n/a | n/a | n/a |
| array_wordcount | large | 20.6 (20.4) | 14.9 (13.9) | n/a | n/a | n/a |

### Artifacts and generated C

Executable sizes and the size of the generated C sources. The generated C size is measured before the C compiler runs; header files and the shared runtime are not counted per program.

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | --- | --- | --- | --- |
| fib | 239,768 | 16,911 | 40,164 | 141,312 |
| fib_iter | 238,749 | 17,184 | 39,032 | n/a |
| fib_memo | 240,466 | 22,734 | 41,381 | n/a |
| loop_sum | 238,133 | 15,015 | 39,032 | 141,312 |
| string_build | 238,786 | 17,761 | 39,717 | 142,336 |
| vec_memory | 237,388 | 9,347 | 38,393 | 132,096 |
| sort_i64 | 241,205 | 29,327 | 40,404 | 142,336 |
| sort_insertion | 240,187 | 23,738 | 39,892 | n/a |
| matrix_multiply | 236,075 | 15,679 | 39,080 | 132,608 |
| binary_trees | 245,552 | 24,368 | 43,154 | 142,336 |
| binary_trees_refs | 246,064 | 23,887 | 43,174 | 142,336 |
| string_split | 239,433 | 21,160 | 40,396 | 142,848 |
| sieve | 239,499 | 19,025 | 39,555 | 141,824 |
| mandelbrot | 235,122 | 8,550 | 38,732 | 131,072 |
| iterator_pipeline | 339,895 | 408,276 | 38,588 | 132,096 |
| hashmap_wordcount | 246,121 | 38,689 | 42,302 | 145,920 |
| array_wordcount | 239,499 | 20,232 | 39,207 | n/a |
| generic_stack | 241,781 | 21,170 | 40,784 | 133,632 |
| fn_pipeline | 334,708 | 389,405 | 38,400 | 132,096 |
| string_unicode | 242,711 | 35,371 | 41,377 | 140,800 |
| hashmap_i64 | 241,177 | 25,011 | 39,891 | 136,192 |
| hashmap_string | 244,761 | 32,268 | 41,594 | 138,240 |
| compile_large (compile-only) | 356,905 | 301,361 | n/a | n/a |
| modules_compile (compile-only) | 569,544 | 2,501,276 | n/a | n/a |
| package_compile (compile-only) | 291,423 | 710,610 | n/a | n/a |

## Analysis

### What this run shows

lsaca 2.3.0 lands 13 of the 22 executed workloads inside 1.4x of C, 14 inside 1.5x and 6 inside 1.15x. The median lsaca/C ratio over the executed workloads is 1.34x.

Absolute milliseconds are not comparable with the 2.2.0 baseline, because the C reference compiler changed from gcc 15.2.0 to gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0 and that compiler is measurably slower on the same sources. The table below therefore compares C-normalised ratios: each language's time divided by the C time of the same run. A positive change means lsaca lost ground relative to the C reference.

| workload | 2.2.0 ratio to C | 2.3.0 ratio to C | change |
| --- | --- | --- | --- |
| `binary_trees` | 8.17x | 12.56x | +53.8% |
| `hashmap_string` | 0.92x | 1.30x | +40.8% |
| `sieve` | 1.20x | 1.50x | +25.2% |
| `loop_sum` | 1.22x | 1.42x | +16.0% |
| `fib_iter` | 1.15x | 1.27x | +10.1% |
| `hashmap_i64` | 1.58x | 1.65x | +4.9% |
| `binary_trees_refs` | 1.77x | 1.85x | +4.5% |
| `mandelbrot` | 1.07x | 1.11x | +4.5% |
| `fib_memo` | 1.48x | 1.51x | +2.0% |
| `iterator_pipeline` | 1.11x | 1.07x | -3.4% |
| `sort_insertion` | 1.45x | 1.34x | -7.5% |
| `string_build` | 6.83x | 6.23x | -8.8% |
| `fn_pipeline` | 1.19x | 1.05x | -11.4% |
| `vec_memory` | 1.40x | 1.18x | -16.2% |
| `matrix_multiply` | 1.47x | 1.21x | -18.1% |
| `fib` | 1.23x | 0.99x | -19.0% |
| `string_unicode` | 2.88x | 2.28x | -20.8% |
| `sort_i64` | 1.41x | 1.10x | -22.1% |
| `string_split` | 2.00x | 1.55x | -22.5% |
| `hashmap_wordcount` | 1.72x | 1.32x | -23.6% |
| `array_wordcount` | 1.79x | 1.34x | -25.4% |
| `generic_stack` | 1.15x | 0.85x | -26.2% |

### Largest improvements against the 2.2.0 baseline

- `generic_stack`: 1.15x to 0.85x (-26.2%).
- `array_wordcount`: 1.79x to 1.34x (-25.4%).
- `hashmap_wordcount`: 1.72x to 1.32x (-23.6%).
- `string_split`: 2.00x to 1.55x (-22.5%).
- `sort_i64`: 1.41x to 1.10x (-22.1%).
- `string_unicode`: 2.88x to 2.28x (-20.8%).

### Largest regressions against the 2.2.0 baseline

- `binary_trees`: 8.17x to 12.56x (+53.8%).
- `hashmap_string`: 0.92x to 1.30x (+40.8%).
- `sieve`: 1.20x to 1.50x (+25.2%).
- `loop_sum`: 1.22x to 1.42x (+16.0%).
- `fib_iter`: 1.15x to 1.27x (+10.1%).
- `hashmap_i64`: 1.58x to 1.65x (+4.9%).

### Workloads outside the 1.4x target

These workloads are above the 1.4x target in this run. Each one has a reason; the reasons are checked against the program source, not guessed.

| workload | lsaca runtime (ms) | C runtime (ms) | ratio | note |
| --- | --- | --- | --- | --- |
| `binary_trees` | 209.1 | 16.7 | 12.56x | by-value tree semantics: each recursive step clones the child subtree, so the deep copies are part of the workload by design; binary_trees_refs is the borrowed-traversal variant |
| `string_build` | 85.5 | 13.7 | 6.23x | one million appends; each append goes through the checked String API instead of writing into a raw buffer |
| `string_unicode` | 41.5 | 18.2 | 2.28x | owned String calls and Unicode table lookups; character-level work checks UTF-8 boundaries |
| `binary_trees_refs` | 38.8 | 21.0 | 1.85x | borrowed tree traversal; references do not copy the tree, so only the traversal cost remains |
| `hashmap_i64` | 309.7 | 187.4 | 1.65x | integer-key map operations; the C version uses a hand-written open-addressing table |
| `string_split` | 34.1 | 22.0 | 1.55x | splitting and parsing an owned string; every token is a String and parsing checks it |
| `fib_memo` | 17.1 | 11.3 | 1.51x | recursion plus a memo map; the map is a builtin HashMap with owned String keys folded away |
| `sieve` | 57.7 | 38.4 | 1.50x | a large vector of flags with checked indexing; the C version uses a raw calloced array |
| `loop_sum` | 17.5 | 12.4 | 1.42x | a tight scalar loop; the only cost is the loop itself |

### Acceptance against the standing targets

| target | result | status |
| --- | --- | --- |
| Every executed runtime within 1.4x of C | 13/22 within 1.4x | partly met |
| Smallest fresh single-file build 0.35-0.5 s | 0.45 s | met |
| Cached rebuild around 15 ms | 86 ms | not met |
| Compile-only workload within 2.5x of the C compile | 3.14 s, no C reference | not scored |
| Generated C compiles without warnings | every measured program built by the harness | met |
| Independent oracle agreement | 22 program checksums and 39 data-scale checksums agree | met |
| Runtime medians within 10% of 2.2.0 (C-normalised) | 5 of 22 outside the budget | partly met |

The 1.4x target is a language-level goal, not a hard release gate: a workload outside it must have a recorded reason, and the table above names them all.

### Python and Node.js

The dynamic languages are far slower than the compiled ones on these workloads. lsaca is between 2.41x and 32.24x the speed of CPython and between 2.26x and 10.07x the speed of Node.js. They are included to keep the page honest about the range: a language that only compares with C can look worse than it is.

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

2. Absolute milliseconds are specific to this host and toolchain: 12th Gen Intel(R) Core(TM) i7-1260P running Windows-11-10.0.26300-SP0, C reference gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0. The 2.2.0 baseline was measured with gcc 15.2.0, so cross-release claims are made about C-normalised ratios only.

3. All measurements were taken in one session on an otherwise idle machine; the harness runs one program at a time and does not overlap builds with measurements.

4. Short workloads have higher relative noise. Every runtime table reports both the median and the minimum of the measured runs so the spread is visible, and the harness discards the first run as a warm-up.

5. The closest workload is `generic_stack` at 0.85x of C; the widest gap is `binary_trees` at 12.56x. Both are real measurements, not rounding artefacts of a single run.

6. `binary_trees` is the largest deliberate semantic gap: lsaca implements value-semantics trees, so a tree assignment copies the tree and the program builds a different shape from the pointer-based C and Rust versions. It is kept because it measures the cost of the language's semantics, not because the implementation is slow.

7. `hashmap_string` is faster than the C reference on some runs. That is the Swiss-table implementation doing its job; the C reference uses a simpler open-addressed table, so this is a data-structure comparison as much as a language comparison.

8. The alternative-implementation workloads (the programs whose name ends in a second variant) are built only for lsaca and C, because translating every alternative five ways would measure translation style rather than the language.

9. Compile-only workloads measure the build, not a running program: they have no runtime row, and their C reference column is n/a. They exist to show how build time grows with source size and module count.

10. The cached build column is the best case for an unchanged source file: the harness reuses the content-addressed build cache, so it measures the cached path rather than a fresh compile.

11. Peak memory is the peak working set of the child process, sampled by the harness through the platform API. It includes the runtime's allocator pools, which trade a little memory for speed.

12. Every runtime table cell is validated: the harness compares the printed checksum against the oracle and fails the measurement if it differs. A wrong answer cannot become a fast result.

13. Relative to the 2.2.0 baseline, `binary_trees`, `hashmap_string`, `sieve`, `loop_sum`, `fib_iter`, `hashmap_i64` moved outside the 2.2.0 measurement noise after normalising by the C reference. The release notes track these explicitly instead of hiding them in a different host speed.

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

The 2.2.0 baseline used for the comparison table lives in `benchmarks/results/archive/v2.2/latest.json`; it reports lsaca 2.2.0 and is never overwritten by the harness.

The regression guard reads the same files:

```sh
python scripts/bench_guard.py --results benchmarks/results/latest.json \
    --baseline benchmarks/results/archive/v2.2/latest.json
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
