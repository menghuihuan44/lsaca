# lsaca cross-language benchmarks

Generated: 2026-11-03T00:20:17+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 2.0.0 (target\bench-tools\lsaca.exe)
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

> lsaca cold build: the default `lsaca build -r`, repeated --compile-repeats times (default 3), each repeat starting from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default. Cold and cached compile times are medians of --compile-repeats runs (default 3); latest.json stores every run plus the minimum and maximum.

> compile_large, modules_compile, package_compile: compile-only workload(s). They measure how the lsaca build time scales with program size and project layout; they are not executed in the runtime or peak-memory tables.

> modules_compile is a multi-module program built from its crate root file, which loads the sibling module files; package_compile is a package with path dependencies built in package mode (no file argument; the manifest is discovered from the package directory).

## Runtime (median (min), milliseconds; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 31.4 (29.8) | 13.4 (12.5) | 10.5 (10.2) | 17.5 (15.2) | 26.8 (25.6) | 17.9 (17.0) | 30.8 (28.2) | 11.4 (11.1) | 18.1 (15.0) | 190.7 (178.5) | 59.5 (46.4) | 24.4 (23.1) | 32.9 (31.4) | 44.0 (36.8) | 14.2 (13.7) | 36.0 (32.6) | 13.7 (12.8) | 18.0 (17.3) | 12.7 (11.2) | 33.3 (32.0) | 212.5 (207.5) | 193.2 (179.8) |
| C (gcc -O2) | 23.8 (22.9) | 7.3 (7.0) | 9.7 (8.7) | 12.4 (11.9) | 11.0 (10.6) | 12.4 (11.8) | 30.9 (28.2) | 8.9 (8.1) | 12.7 (12.1) | 18.5 (17.6) | 25.5 (24.2) | 12.8 (12.3) | 29.6 (27.4) | 26.6 (23.6) | 12.3 (11.4) | 25.2 (22.6) | 9.0 (7.7) | 13.6 (13.4) | 9.1 (8.2) | 10.9 (10.2) | 150.2 (137.3) | 180.5 (170.8) |
| Rust (rustc -O) | 19.2 (17.9) | n/a | n/a | 9.8 (8.9) | 59.4 (56.2) | 13.5 (13.2) | 35.2 (28.4) | n/a | 10.9 (9.3) | 17.6 (16.8) | 33.4 (28.3) | 16.7 (15.1) | 28.2 (27.1) | 29.8 (28.3) | 12.1 (11.8) | 46.2 (43.9) | n/a | 15.6 (13.4) | 9.7 (8.8) | 14.6 (12.9) | 187.6 (181.7) | 238.1 (220.0) |
| Python 3 | 941.8 (903.4) | n/a | n/a | 856.4 (751.6) | 371.2 (347.8) | 506.5 (467.6) | 558.0 (522.8) | n/a | 1207.1 (1053.3) | 103.4 (99.5) | 116.3 (104.9) | 100.2 (73.5) | 509.1 (493.5) | 2443.8 (2304.0) | 207.4 (201.8) | 109.2 (107.9) | n/a | 306.5 (297.7) | 36.0 (35.1) | 239.2 (232.7) | 1942.3 (1915.0) | 1020.9 (1007.2) |
| Node.js | 184.4 (176.6) | n/a | n/a | 127.4 (117.4) | 217.1 (204.5) | 189.9 (183.8) | 188.3 (159.6) | n/a | 142.0 (127.0) | 136.3 (129.3) | 135.9 (124.5) | 160.1 (153.6) | 160.6 (151.2) | 138.0 (121.9) | 159.1 (138.4) | 129.3 (119.2) | n/a | 138.5 (133.2) | 116.8 (108.3) | 172.4 (169.7) | 705.7 (695.8) | 404.4 (387.9) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 5.1 | 5.1 | 5.1 | 5.1 | 11.7 | 20.1 | 7.9 | 5.1 | 5.5 | 53.1 | 17.0 | 10.6 | 20.1 | 5.1 | 12.4 | 5.4 | 5.1 | 13.8 | 5.1 | 6.4 | 56.7 | 30.8 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.0 | 4.5 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.3 | 52.4 | 19.7 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.4 | 18.9 | 5.2 | n/a | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.1 | 11.2 | 4.4 | n/a | 12.6 | 4.1 | 4.7 | 55.9 | 19.8 |
| Python 3 | 11.0 | n/a | n/a | 10.9 | 72.2 | 142.8 | 21.3 | n/a | 13.6 | 21.1 | 21.1 | 17.0 | 26.2 | 11.0 | 49.2 | 11.4 | n/a | 54.2 | 11.0 | 27.0 | 113.5 | 37.9 |
| Node.js | 43.0 | n/a | n/a | 45.9 | 88.3 | 121.2 | 50.8 | n/a | 46.8 | 56.6 | 56.5 | 51.0 | 59.2 | 44.5 | 58.1 | 46.4 | n/a | 72.1 | 40.1 | 68.3 | 102.5 | 90.4 |

## Runtime by data scale (median (min), milliseconds; lower is better)

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

> The medium scale reuses the primary measurement above (default arguments); the small and large scales pass explicit arguments, listed under Data scales below.

## Artifact sizes (bytes)

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

