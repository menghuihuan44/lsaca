# lsaca cross-language benchmarks

Generated: 2026-11-27T21:38:17+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 2.4.0 (target\bench-tools\lsaca.exe)
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

> lsaca cold build: the default `lsaca build -r`, repeated --compile-repeats times (default 3), each repeat starting from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default. Cold and cached compile times are medians of --compile-repeats runs (default 3); latest.json stores every run plus the minimum and maximum.

> compile_large, modules_compile, package_compile: compile-only workload(s). They measure how the lsaca build time scales with program size and project layout; they are not executed in the runtime or peak-memory tables.

> modules_compile is a multi-module program built from its crate root file, which loads the sibling module files; package_compile is a package with path dependencies built in package mode (no file argument; the manifest is discovered from the package directory).

## Runtime (median (min), milliseconds; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 38.6 (35.2) | 16.1 (14.4) | 17.7 (16.2) | 23.4 (20.0) | 99.7 (93.9) | 27.4 (24.0) | 44.8 (39.8) | 17.9 (15.8) | 23.2 (21.8) | 321.7 (294.4) | 42.7 (39.8) | 31.2 (28.2) | 47.8 (46.2) | 44.3 (37.5) | 20.1 (18.4) | 43.1 (36.4) | 19.3 (17.5) | 23.2 (22.0) | 17.0 (15.3) | 42.4 (41.2) | 355.6 (309.1) | 263.0 (255.2) |
| C (gcc -O2) | 32.2 (30.7) | 16.3 (13.6) | 17.2 (16.0) | 19.6 (18.1) | 20.9 (18.8) | 22.7 (20.5) | 42.5 (40.8) | 14.4 (13.4) | 21.7 (20.3) | 28.0 (27.2) | 23.9 (22.7) | 26.7 (20.6) | 43.0 (41.7) | 43.7 (41.5) | 13.6 (13.3) | 32.5 (28.7) | 18.4 (15.7) | 20.5 (19.8) | 13.8 (12.6) | 23.6 (21.7) | 227.7 (223.3) | 242.3 (225.7) |
| Rust (rustc -O) | 18.6 (16.3) | n/a | n/a | 16.1 (14.7) | 83.1 (77.6) | 19.0 (18.9) | 46.0 (41.9) | n/a | 18.9 (16.7) | 34.2 (30.9) | 25.6 (24.5) | 20.0 (19.0) | 39.8 (37.4) | 33.7 (30.8) | 15.7 (15.3) | 52.5 (47.2) | n/a | 19.5 (18.6) | 15.5 (14.1) | 20.7 (18.6) | 309.0 (297.7) | 248.6 (243.7) |
| Python 3 | 1083.3 (1048.7) | n/a | n/a | 387.3 (352.0) | 248.0 (217.7) | 289.7 (265.5) | 420.2 (395.3) | n/a | 547.3 (512.9) | 150.4 (129.8) | 102.1 (100.8) | 82.9 (66.4) | 266.0 (255.5) | 822.5 (777.6) | 137.7 (123.8) | 90.3 (75.9) | n/a | 248.8 (244.3) | 50.2 (45.3) | 232.6 (183.9) | 1611.7 (1310.9) | 635.5 (614.1) |
| Node.js | 188.3 (177.8) | n/a | n/a | 147.7 (139.3) | 143.9 (138.3) | 177.7 (161.2) | 137.1 (128.7) | n/a | 122.1 (118.9) | 136.4 (132.2) | 91.8 (88.9) | 105.9 (100.6) | 119.3 (109.4) | 93.7 (91.2) | 101.3 (97.8) | 90.5 (87.3) | n/a | 101.8 (95.1) | 95.8 (89.1) | 193.8 (182.9) | 906.0 (696.5) | 466.3 (423.5) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.5 | 4.5 | 4.5 | 4.5 | 11.0 | 19.2 | 7.1 | 4.5 | 4.6 | 52.3 | 16.2 | 9.8 | 19.3 | 4.5 | 11.6 | 4.7 | 4.5 | 13.0 | 4.5 | 5.6 | 55.9 | 30.0 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.2 | 18.8 | 5.0 | 4.0 | 4.3 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.7 | 52.5 | 19.6 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.4 | 18.9 | 5.1 | n/a | 4.4 | 7.7 | 7.7 | 4.3 | 18.9 | 4.1 | 11.2 | 4.3 | n/a | 12.6 | 4.1 | 4.7 | 55.7 | 19.7 |
| Python 3 | 10.9 | n/a | n/a | 10.9 | 74.4 | 142.0 | 20.2 | n/a | 13.5 | 21.0 | 21.0 | 17.0 | 26.1 | 11.0 | 49.2 | 11.3 | n/a | 54.1 | 11.0 | 26.9 | 113.3 | 38.1 |
| Node.js | 43.1 | n/a | n/a | 45.5 | 93.5 | 128.0 | 51.7 | n/a | 47.2 | 52.5 | 52.7 | 53.5 | 59.8 | 45.4 | 56.0 | 46.7 | n/a | 74.8 | 40.5 | 65.4 | 116.5 | 91.5 |

## Runtime by data scale (median (min), milliseconds; lower is better)

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
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

> The medium scale reuses the primary measurement above (default arguments); the small and large scales pass explicit arguments, listed under Data scales below.

## Artifact sizes (bytes)

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | ---: | ---: | ---: | ---: |
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

