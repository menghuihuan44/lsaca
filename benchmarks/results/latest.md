# lsaca cross-language benchmarks

Generated: 2026-11-06T18:46:10+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 2.1.0 (target\bench-tools\lsaca.exe)
- lsaca build backend: direct C compiler back end (default; --cmake available)
- gcc: gcc.EXE (Rev14, Built by MSYS2 project) 15.2.0
- rustc: rustc 1.99.0 (b940084d7 2026-09-28)
- Python: Python 3.14.4
- Node.js: v24.14.1
- cmake: cmake version 4.1.0
- ninja: 1.13.2
- lsaca build mode: official

## Compile time (seconds, median of 3 runs; lower is better)

| program | lsaca cold build (default back end) | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| fib | 0.564 | 0.055 | n/a | 0.263 | 0.334 | n/a | n/a |
| fib_iter | 0.525 | 0.095 | n/a | 0.248 | n/a | n/a | n/a |
| fib_memo | 0.585 | 0.060 | n/a | 0.264 | n/a | n/a | n/a |
| loop_sum | 0.489 | 0.065 | n/a | 0.237 | 0.395 | n/a | n/a |
| string_build | 0.502 | 0.063 | n/a | 0.267 | 0.386 | n/a | n/a |
| vec_memory | 0.473 | 0.061 | n/a | 0.255 | 0.314 | n/a | n/a |
| sort_i64 | 0.630 | 0.108 | n/a | 0.367 | 0.418 | n/a | n/a |
| sort_insertion | 0.771 | 0.062 | n/a | 0.255 | n/a | n/a | n/a |
| matrix_multiply | 0.474 | 0.058 | n/a | 0.269 | 0.368 | n/a | n/a |
| binary_trees | 0.585 | 0.059 | n/a | 0.323 | 0.398 | n/a | n/a |
| binary_trees_refs | 0.660 | 0.066 | n/a | 0.474 | 0.471 | n/a | n/a |
| string_split | 0.818 | 0.085 | n/a | 0.281 | 0.424 | n/a | n/a |
| sieve | 0.692 | 0.065 | n/a | 0.248 | 0.419 | n/a | n/a |
| mandelbrot | 0.557 | 0.075 | n/a | 0.275 | 0.256 | n/a | n/a |
| iterator_pipeline | 1.671 | 0.064 | n/a | 0.225 | 0.253 | n/a | n/a |
| hashmap_wordcount | 0.516 | 0.055 | n/a | 0.252 | 0.412 | n/a | n/a |
| array_wordcount | 0.458 | 0.053 | n/a | 0.217 | n/a | n/a | n/a |
| generic_stack | 0.499 | 0.057 | n/a | 0.232 | 0.267 | n/a | n/a |
| fn_pipeline | 1.569 | 0.056 | n/a | 0.228 | 0.252 | n/a | n/a |
| string_unicode | 0.589 | 0.073 | n/a | 0.382 | 0.444 | n/a | n/a |
| hashmap_i64 | 0.527 | 0.055 | n/a | 0.265 | 0.481 | n/a | n/a |
| hashmap_string | 0.512 | 0.055 | n/a | 0.260 | 0.455 | n/a | n/a |
| compile_large (compile-only) | 1.963 | 0.549 | n/a | n/a | n/a | n/a | n/a |
| modules_compile (compile-only) | 2.961 | 0.510 | n/a | n/a | n/a | n/a | n/a |
| package_compile (compile-only) | 1.332 | 0.116 | n/a | n/a | n/a | n/a | n/a |

> lsaca cold build: the default `lsaca build -r`, repeated --compile-repeats times (default 3), each repeat starting from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default. Cold and cached compile times are medians of --compile-repeats runs (default 3); latest.json stores every run plus the minimum and maximum.

> compile_large, modules_compile, package_compile: compile-only workload(s). They measure how the lsaca build time scales with program size and project layout; they are not executed in the runtime or peak-memory tables.

> modules_compile is a multi-module program built from its crate root file, which loads the sibling module files; package_compile is a package with path dependencies built in package mode (no file argument; the manifest is discovered from the package directory).

## Runtime (median (min), milliseconds; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 27.6 (25.8) | 24.1 (16.3) | 13.6 (13.1) | 20.6 (17.7) | 97.2 (86.0) | 19.9 (18.9) | 56.6 (53.1) | 14.3 (13.9) | 16.2 (15.1) | 212.5 (198.6) | 38.2 (35.7) | 39.3 (38.9) | 37.8 (34.8) | 39.5 (37.5) | 16.0 (15.7) | 32.5 (30.3) | 13.1 (12.8) | 17.2 (16.8) | 12.1 (11.9) | 38.1 (37.2) | 196.3 (194.1) | 181.8 (167.4) |
| C (gcc -O2) | 23.7 (22.4) | 9.2 (8.4) | 10.2 (9.9) | 13.4 (12.8) | 13.0 (12.4) | 14.5 (14.3) | 33.1 (30.7) | 9.8 (9.4) | 12.7 (11.8) | 19.2 (18.9) | 26.2 (24.1) | 14.3 (13.7) | 29.3 (27.5) | 31.7 (29.7) | 12.2 (12.1) | 21.7 (20.9) | 9.0 (8.7) | 13.9 (13.2) | 8.3 (8.1) | 15.6 (13.6) | 145.6 (144.4) | 163.5 (152.8) |
| Rust (rustc -O) | 11.1 (10.6) | n/a | n/a | 13.0 (11.2) | 59.5 (57.7) | 18.5 (17.6) | 29.3 (26.9) | n/a | 21.7 (16.1) | 22.2 (21.3) | 20.0 (18.6) | 13.2 (12.9) | 30.7 (29.3) | 31.1 (29.7) | 12.3 (11.7) | 41.9 (40.0) | n/a | 15.0 (13.8) | 8.9 (8.6) | 14.3 (13.8) | 181.2 (179.6) | 213.6 (208.6) |
| Python 3 | 841.8 (827.9) | n/a | n/a | 901.9 (836.8) | 418.1 (375.1) | 495.7 (464.0) | 661.5 (594.6) | n/a | 1385.9 (1262.2) | 131.8 (114.9) | 102.8 (98.5) | 70.4 (65.5) | 594.6 (493.1) | 2508.7 (2179.4) | 187.8 (186.9) | 99.9 (96.4) | n/a | 282.3 (266.4) | 32.8 (31.2) | 245.1 (220.4) | 1751.0 (1732.5) | 905.1 (891.6) |
| Node.js | 204.2 (193.3) | n/a | n/a | 129.4 (119.9) | 201.0 (194.1) | 221.7 (211.9) | 161.8 (151.2) | n/a | 132.9 (122.8) | 163.4 (142.2) | 138.2 (133.8) | 189.0 (157.4) | 184.2 (159.4) | 127.7 (124.5) | 132.8 (131.2) | 132.8 (127.7) | n/a | 132.3 (129.7) | 117.5 (107.4) | 185.0 (176.7) | 634.2 (620.1) | 343.2 (338.8) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 5.1 | 5.1 | 5.1 | 5.1 | 11.9 | 20.1 | 7.9 | 5.2 | 5.5 | 53.1 | 17.1 | 10.6 | 20.1 | 5.1 | 12.5 | 5.5 | 5.1 | 13.9 | 5.2 | 6.4 | 56.8 | 30.8 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.0 | 4.6 | 7.7 | 7.7 | 4.0 | 18.8 | 4.0 | 11.2 | 4.3 | 4.1 | 12.5 | 4.0 | 4.7 | 52.6 | 19.7 |
| Rust (rustc -O) | 4.2 | n/a | n/a | 4.2 | 7.4 | 18.9 | 5.2 | n/a | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 | 11.3 | 4.4 | n/a | 12.6 | 4.2 | 4.7 | 55.8 | 19.8 |
| Python 3 | 11.0 | n/a | n/a | 11.0 | 74.4 | 142.8 | 21.4 | n/a | 13.6 | 21.1 | 21.0 | 16.9 | 26.3 | 11.1 | 49.3 | 11.4 | n/a | 54.2 | 11.1 | 26.0 | 113.4 | 38.0 |
| Node.js | 43.0 | n/a | n/a | 45.8 | 88.2 | 121.3 | 51.0 | n/a | 47.0 | 56.1 | 56.8 | 51.5 | 59.1 | 44.6 | 57.9 | 46.4 | n/a | 72.4 | 40.0 | 68.6 | 102.8 | 90.5 |

## Runtime by data scale (median (min), milliseconds; lower is better)

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| fib | small | 12.3 (12.0) | 9.7 (9.3) | 8.8 (8.6) | 39.2 (37.4) | 140.2 (137.3) |
| fib | medium | 27.6 (25.8) | 23.7 (22.4) | 11.1 (10.6) | 841.8 (827.9) | 204.2 (193.3) |
| fib | large | 38.1 (37.2) | 33.3 (31.2) | 14.0 (14.0) | 1362.6 (1337.6) | 266.2 (262.0) |
| fib_iter | small | 15.5 (15.0) | 11.0 (9.1) | n/a | n/a | n/a |
| fib_iter | medium | 24.1 (16.3) | 9.2 (8.4) | n/a | n/a | n/a |
| fib_iter | large | 16.0 (15.8) | 10.9 (10.5) | n/a | n/a | n/a |
| fib_memo | small | 14.7 (13.9) | 10.3 (9.4) | n/a | n/a | n/a |
| fib_memo | medium | 13.6 (13.1) | 10.2 (9.9) | n/a | n/a | n/a |
| fib_memo | large | 13.7 (13.0) | 9.9 (9.6) | n/a | n/a | n/a |
| loop_sum | small | 14.4 (13.8) | 10.1 (9.7) | 11.8 (11.3) | 143.9 (142.2) | 114.3 (106.0) |
| loop_sum | medium | 20.6 (17.7) | 13.4 (12.8) | 13.0 (11.2) | 901.9 (836.8) | 129.4 (119.9) |
| loop_sum | large | 20.7 (20.7) | 14.0 (13.5) | 14.0 (13.7) | 1559.5 (1314.0) | 117.2 (117.0) |
| string_build | small | 21.2 (20.3) | 10.2 (10.0) | 9.7 (9.2) | 50.1 (48.9) | 155.4 (138.1) |
| string_build | medium | 97.2 (86.0) | 13.0 (12.4) | 59.5 (57.7) | 418.1 (375.1) | 201.0 (194.1) |
| string_build | large | 408.5 (358.2) | 23.5 (23.4) | 218.0 (217.6) | 1465.4 (1422.4) | 565.3 (430.9) |
| sort_i64 | small | 16.7 (16.2) | 10.3 (10.2) | 9.7 (9.2) | 35.9 (35.9) | 111.8 (110.3) |
| sort_i64 | medium | 56.6 (53.1) | 33.1 (30.7) | 29.3 (26.9) | 661.5 (594.6) | 161.8 (151.2) |
| sort_i64 | large | 90.1 (85.8) | 65.7 (61.7) | 57.6 (56.9) | 1635.1 (1445.5) | 269.5 (267.4) |
| sort_insertion | small | 13.4 (12.6) | 10.2 (9.8) | n/a | n/a | n/a |
| sort_insertion | medium | 14.3 (13.9) | 9.8 (9.4) | n/a | n/a | n/a |
| sort_insertion | large | 16.6 (15.9) | 12.0 (11.8) | n/a | n/a | n/a |
| binary_trees | small | 23.0 (22.0) | 9.8 (9.1) | 13.3 (12.5) | 54.7 (51.5) | 126.9 (122.9) |
| binary_trees | medium | 212.5 (198.6) | 19.2 (18.9) | 22.2 (21.3) | 131.8 (114.9) | 163.4 (142.2) |
| binary_trees | large | 1183.4 (956.6) | 48.9 (47.5) | 61.6 (56.0) | 372.3 (354.6) | 171.4 (170.8) |
| binary_trees_refs | small | 20.3 (18.7) | 15.1 (14.8) | 9.7 (9.5) | 38.8 (37.6) | 127.0 (122.8) |
| binary_trees_refs | medium | 38.2 (35.7) | 26.2 (24.1) | 20.0 (18.6) | 102.8 (98.5) | 138.2 (133.8) |
| binary_trees_refs | large | 120.9 (116.3) | 65.1 (63.3) | 49.5 (49.4) | 297.1 (295.4) | 198.9 (195.1) |
| string_split | small | 29.8 (28.6) | 10.8 (10.7) | 11.0 (11.0) | 38.2 (36.4) | 156.0 (139.1) |
| string_split | medium | 39.3 (38.9) | 14.3 (13.7) | 13.2 (12.9) | 70.4 (65.5) | 189.0 (157.4) |
| string_split | large | 151.3 (137.2) | 48.6 (47.0) | 45.7 (45.0) | 388.8 (375.1) | 322.9 (297.8) |
| sieve | small | 15.6 (15.2) | 10.0 (9.5) | 14.8 (13.7) | 96.4 (85.7) | 148.0 (142.5) |
| sieve | medium | 37.8 (34.8) | 29.3 (27.5) | 30.7 (29.3) | 594.6 (493.1) | 184.2 (159.4) |
| sieve | large | 89.7 (89.2) | 81.8 (80.2) | 87.4 (83.2) | 1690.6 (1433.7) | 283.2 (270.4) |
| hashmap_wordcount | small | 14.3 (14.1) | 9.3 (8.9) | 11.9 (11.2) | 35.4 (34.8) | 111.7 (110.0) |
| hashmap_wordcount | medium | 32.5 (30.3) | 21.7 (20.9) | 41.9 (40.0) | 99.9 (96.4) | 132.8 (127.7) |
| hashmap_wordcount | large | 108.2 (104.2) | 76.9 (74.5) | 164.0 (161.2) | 359.2 (349.6) | 185.5 (181.1) |
| array_wordcount | small | 12.3 (11.9) | 9.2 (8.9) | n/a | n/a | n/a |
| array_wordcount | medium | 13.1 (12.8) | 9.0 (8.7) | n/a | n/a | n/a |
| array_wordcount | large | 15.6 (15.3) | 9.3 (8.9) | n/a | n/a | n/a |

> The medium scale reuses the primary measurement above (default arguments); the small and large scales pass explicit arguments, listed under Data scales below.

## Artifact sizes (bytes)

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | ---: | ---: | ---: | ---: |
| fib | 335,640 | 16,903 | 141,002 | 141,312 |
| fib_iter | 334,621 | 17,176 | 139,870 | n/a |
| fib_memo | 336,338 | 22,684 | 142,044 | n/a |
| loop_sum | 334,517 | 15,011 | 139,870 | 141,312 |
| string_build | 334,658 | 17,757 | 140,382 | 142,336 |
| vec_memory | 333,260 | 9,347 | 139,058 | 132,096 |
| sort_i64 | 337,077 | 29,287 | 140,557 | 142,336 |
| sort_insertion | 336,059 | 23,702 | 140,557 | n/a |
| matrix_multiply | 332,459 | 15,659 | 139,745 | 132,608 |
| binary_trees | 339,888 | 24,364 | 143,819 | 142,336 |
| binary_trees_refs | 340,400 | 23,883 | 143,839 | 142,336 |
| string_split | 335,305 | 21,156 | 142,849 | 142,848 |
| sieve | 335,371 | 19,013 | 140,045 | 141,824 |
| mandelbrot | 330,994 | 8,550 | 139,058 | 131,072 |
| iterator_pipeline | 420,445 | 329,196 | 139,253 | 132,096 |
| hashmap_wordcount | 339,418 | 28,533 | 143,556 | 145,920 |
| array_wordcount | 335,371 | 20,216 | 140,045 | n/a |
| generic_stack | 337,653 | 21,170 | 142,725 | 133,632 |
| fn_pipeline | 415,258 | 310,282 | 139,238 | 132,096 |
| string_unicode | 339,095 | 35,371 | 142,035 | 140,800 |
| hashmap_i64 | 336,597 | 22,382 | 140,381 | 136,192 |
| hashmap_string | 339,145 | 29,325 | 143,360 | 138,240 |
| compile_large (compile-only) | 452,777 | 301,361 | n/a | n/a |
| modules_compile (compile-only) | 646,579 | 2,321,642 | n/a | n/a |
| package_compile (compile-only) | 386,831 | 680,334 | n/a | n/a |

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

