# lsaca cross-language benchmarks

Generated: 2026-11-28T17:14:30+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 2.5.0 (target\bench-tools\lsaca.exe)
- lsaca build backend: direct C compiler back end (default; --cmake available)
- gcc: gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0
- rustc: rustc 1.100.0 (0ef1bc386 2026-11-09)
- Python: Python 3.14.10
- Node.js: v26.13.0
- cmake: cmake version 4.1.0
- ninja: 1.13.2
- lsaca build mode: official

## Compile time (seconds, median of 3 runs; lower is better)

| program | lsaca cold build (default back end) | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
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

> lsaca cold build: the default `lsaca build -r`, repeated --compile-repeats times (default 3), each repeat starting from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default. Cold and cached compile times are medians of --compile-repeats runs (default 3); latest.json stores every run plus the minimum and maximum.

> compile_large, modules_compile, package_compile: compile-only workload(s). They measure how the lsaca build time scales with program size and project layout; they are not executed in the runtime or peak-memory tables.

> modules_compile is a multi-module program built from its crate root file, which loads the sibling module files; package_compile is a package with path dependencies built in package mode (no file argument; the manifest is discovered from the package directory).

## Runtime (median (min), milliseconds; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 34.4 (31.5) | 19.0 (17.7) | 15.0 (13.0) | 22.8 (19.6) | 124.0 (110.1) | 25.8 (24.8) | 44.7 (42.7) | 21.3 (20.3) | 21.7 (20.2) | 332.7 (297.5) | 52.4 (47.5) | 34.4 (33.3) | 54.4 (50.6) | 42.4 (38.2) | 20.0 (19.5) | 42.6 (40.7) | 19.5 (18.2) | 24.2 (22.8) | 14.9 (14.1) | 40.5 (36.4) | 276.9 (269.8) | 225.3 (210.9) |
| C (gcc -O2) | 31.7 (29.9) | 16.7 (14.3) | 16.2 (14.3) | 17.3 (16.8) | 26.5 (20.8) | 28.2 (24.1) | 45.0 (41.7) | 14.6 (13.9) | 20.6 (19.5) | 23.4 (22.4) | 29.3 (25.6) | 19.1 (18.2) | 36.6 (32.6) | 38.2 (35.6) | 19.0 (18.0) | 35.5 (32.7) | 21.1 (19.1) | 23.0 (21.4) | 18.6 (16.9) | 16.3 (15.9) | 174.3 (173.4) | 207.2 (188.2) |
| Rust (rustc -O) | 20.8 (19.6) | n/a | n/a | 17.1 (15.9) | 104.7 (82.8) | 25.1 (23.4) | 50.9 (46.0) | n/a | 18.6 (17.7) | 26.4 (23.9) | 27.4 (27.0) | 25.2 (22.8) | 37.9 (35.8) | 39.0 (36.1) | 19.1 (18.2) | 57.8 (56.5) | n/a | 19.5 (18.3) | 10.6 (10.4) | 18.4 (16.5) | 205.5 (197.4) | 247.6 (223.5) |
| Python 3 | 1243.4 (1110.4) | n/a | n/a | 426.1 (383.6) | 234.9 (228.9) | 350.9 (323.4) | 545.3 (425.6) | n/a | 627.5 (558.6) | 101.4 (98.1) | 112.8 (104.7) | 91.2 (78.3) | 264.5 (259.9) | 1034.9 (894.2) | 150.6 (142.2) | 90.6 (83.8) | n/a | 254.1 (246.7) | 37.1 (33.3) | 156.3 (153.5) | 1195.9 (1165.1) | 582.2 (548.3) |
| Node.js | 190.8 (169.5) | n/a | n/a | 136.2 (131.8) | 161.4 (150.0) | 213.3 (181.8) | 142.0 (138.2) | n/a | 130.0 (121.5) | 115.0 (103.4) | 98.0 (96.1) | 161.7 (143.0) | 129.2 (113.5) | 149.7 (136.0) | 131.0 (128.1) | 119.4 (113.2) | n/a | 122.8 (89.5) | 85.4 (80.9) | 132.7 (128.8) | 676.8 (647.3) | 539.0 (512.4) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.5 | 4.5 | 4.5 | 4.5 | 11.0 | 19.2 | 7.0 | 4.5 | 4.6 | 52.3 | 16.2 | 9.8 | 19.3 | 4.5 | 11.6 | 4.7 | 4.5 | 13.0 | 4.5 | 5.6 | 55.9 | 29.9 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.2 | 18.8 | 5.0 | 4.0 | 4.3 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.7 | 52.5 | 19.6 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.3 | 18.8 | 5.1 | n/a | 4.4 | 7.6 | 7.6 | 4.3 | 18.9 | 4.1 | 11.2 | 4.3 | n/a | 12.6 | 4.1 | 4.7 | 55.8 | 19.7 |
| Python 3 | 10.9 | n/a | n/a | 10.9 | 73.6 | 142.6 | 20.9 | n/a | 13.5 | 20.9 | 20.9 | 16.9 | 26.1 | 10.9 | 49.2 | 11.3 | n/a | 54.1 | 10.9 | 25.4 | 113.3 | 38.0 |
| Node.js | 43.0 | n/a | n/a | 45.6 | 93.4 | 128.4 | 51.4 | n/a | 47.3 | 52.5 | 52.3 | 53.7 | 59.7 | 46.0 | 56.2 | 46.7 | n/a | 74.8 | 40.5 | 65.0 | 116.7 | 91.5 |

## Runtime by data scale (median (min), milliseconds; lower is better)

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
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

> The medium scale reuses the primary measurement above (default arguments); the small and large scales pass explicit arguments, listed under Data scales below.

## Artifact sizes (bytes)

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | ---: | ---: | ---: | ---: |
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

## Independent oracles

> scripts/oracles.py reimplements every executed workload and recomputes its checksum line without running lsaca: 22 program checksums and 39 data-scale checksums agree with the expected output (mismatches: 0).

## Workloads

- fib: recursive fibonacci(30), called 10 times, checksummed; expected stdout checksum=8320400
- fib_iter: iterative fibonacci(30) with two rolling values, called 10 times; alternative implementation with the same checksum as fib; expected stdout checksum=8320400
- fib_memo: recursive fibonacci(30) with a memo table, called 10 times; alternative implementation with the same checksum as fib; expected stdout checksum=8320400
- loop_sum: integer sum 1..=8,000,000; expected stdout checksum=32000004000000
- string_build: append 1,000,000 numeric tokens (i % 1000) to a string; checksum is length,sum-of-token-values; expected stdout checksum=3890000,499500000
- vec_memory: build a vector of 1,000,000 Point{x=i64, y=i64}, then checksum; expected stdout checksum=1499998500000
- sort_i64: sort 200,000 pseudo-random i64 values with an iterative quicksort written in the language; checksum is first,last,sum; expected stdout checksum=21749,2147481261,214813518297651
- sort_insertion: insertion sort over the same 1,000 pseudo-random i64 values as sort_i64; alternative implementation for small inputs, checksum is first,last,sum; expected stdout checksum=91125,2140623151,1054904362226
- matrix_multiply: multiply two 160x160 i64 matrices with the naive triple loop; checksum is the sum of the result entries; expected stdout checksum=1018924712000
- binary_trees: build a perfect binary tree of depth 16 (131,071 nodes) and sum all payloads; expected stdout checksum=2147450880
- binary_trees_refs: the same perfect binary tree of depth 16 (131,071 nodes) as binary_trees, but summed through borrowed references (&Option<Box<Node>>) instead of by value, so the traversal does not copy the tree; checksum matches binary_trees; expected stdout checksum=2147450880
- string_split: build a 194,500-byte comma-separated numeric string, split it on commas, parse every token and sum; checksum is len,count,sum; expected stdout checksum=194500,50000,24975000
- sieve: sieve of Eratosthenes: count the primes below 2,000,000; expected stdout checksum=148933
- mandelbrot: 600x400 Mandelbrot escape-time grid, at most 100 iterations per pixel; checksum is the sum of iteration counts; expected stdout checksum=6282265
- iterator_pipeline: run 1,000,000 i64 values through the lazy chain filter(even).map(x * x).fold(0, add) and print the folded sum; measures iterator adapter overhead against a hand-written loop; expected stdout checksum=166666166667000000
- hashmap_wordcount: count 200,000 deterministic words drawn from a 4,096-word alphabet in a hash map, then fold the squared counts; checksum is the sum of squared counts, the number of distinct keys and the highest frequency (order-independent); expected stdout checksum=9766208,4096,49
- array_wordcount: count the same 200,000 deterministic words as hashmap_wordcount in a flat indexed array of 4,096 counters instead of a hash map; alternative implementation, same checksum; expected stdout checksum=9766208,4096,49
- generic_stack: the same generic stack instantiated for i64 and for String: push/pop 1,000,000 integers and 1,000 strings; checksum is integer sum and total string byte length; expected stdout checksum=499999500000,5900
- fn_pipeline: rule records that carry a label String and a fn(i64) -> i64 value in one struct, applied 1,000 times each plus a clone of the first rule; checksum is the result sum and total label byte length; expected stdout checksum=53637,22
- string_unicode: build a 120,000-scalar mixed ASCII/Unicode string one char at a time, fold every scalar from iteration and from 1,000 random char_at accesses, apply full-Unicode to_uppercase, slice on character boundaries, push/pop and extend; checksum is char count, byte length and the folded scalar checksum; expected stdout checksum=3357,6921,368454760
- hashmap_i64: 1,000,000 integer-key inserts, 1,000,000 present lookups, 1,000,000 absent lookups and 100,000 deletions in a hash map; checksum is the folded value sum, the final length and the number of successful lookups; expected stdout checksum=821060469,900000,1000000
- hashmap_string: 200,000 distinct generated String keys inserted into a hash map, then 200,000 present lookups, 200,000 absent lookups (folding the bytes of each absent key) and 20,000 deletions; checksum is the folded value sum, the final length and the number of successful lookups; expected stdout checksum=794025725,180000,200000
- compile_large: generated 4,511-line lsaca source (450 functions) used to measure how the lsaca build time scales with program size; compile-only (expected stdout checksum=14655037, not measured by this harness)
- modules_compile: multi-module program: one crate root (main.ls) plus eight sibling module files, about 3,600 lines in total, with structs, enums, generics, trait impls, strings, vectors and hash maps; used to measure the multi-file build path; compile-only (expected stdout checksum=547598677, not measured by this harness)
- package_compile: package with path dependencies: the binary package compile_app plus the library packages util_a and util_b (util_b depends on util_a), about 1,090 lines spread over thirteen files, used to measure package-mode builds; compile-only (expected stdout checksum=417367897, not measured by this harness)

## Data scales

- fib: small: args 20 10 -> checksum=67650; medium: args (default) -> checksum=8320400; large: args 31 10 -> checksum=13462690
- fib_iter: small: args 20 10 -> checksum=67650; medium: args (default) -> checksum=8320400; large: args 31 10 -> checksum=13462690
- fib_memo: small: args 20 10 -> checksum=67650; medium: args (default) -> checksum=8320400; large: args 31 10 -> checksum=13462690
- loop_sum: small: args 1000000 -> checksum=500000500000; medium: args (default) -> checksum=32000004000000; large: args 12000000 -> checksum=72000006000000
- string_build: small: args 10000 -> checksum=38900,4995000; medium: args (default) -> checksum=3890000,499500000; large: args 4000000 -> checksum=15560000,1998000000
- sort_i64: small: args 1000 -> checksum=91125,2140623151,1054904362226; medium: args (default) -> checksum=21749,2147481261,214813518297651; large: args 500000 -> checksum=9862,2147481261,537547986243832
- sort_insertion: small: args 250 -> checksum=5639553,2140623151,264749012456; medium: args (default) -> checksum=91125,2140623151,1054904362226; large: args 4000 -> checksum=30903,2147411439,4335208945477
- binary_trees: small: args 10 -> checksum=523776; medium: args (default) -> checksum=2147450880; large: args 18 -> checksum=34359607296
- binary_trees_refs: small: args 10 -> checksum=523776; medium: args (default) -> checksum=2147450880; large: args 18 -> checksum=34359607296
- string_split: small: args 1000 -> checksum=3890,1000,499500; medium: args (default) -> checksum=194500,50000,24975000; large: args 500000 -> checksum=1945000,500000,249750000
- sieve: small: args 100000 -> checksum=9592; medium: args (default) -> checksum=148933; large: args 5000000 -> checksum=348513
- hashmap_wordcount: small: args 10000 -> checksum=25424,4096,3; medium: args (default) -> checksum=9766208,4096,49; large: args 1000000 -> checksum=244141120,4096,245
- array_wordcount: small: args 10000 -> checksum=25424,4096,3; medium: args (default) -> checksum=9766208,4096,49; large: args 1000000 -> checksum=244141120,4096,245

