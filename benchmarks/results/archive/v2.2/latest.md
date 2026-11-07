# lsaca cross-language benchmarks

Generated: 2026-11-07T17:21:34+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 2.2.0 (target\bench-tools\lsaca.exe)
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

> lsaca cold build: the default `lsaca build -r`, repeated --compile-repeats times (default 3), each repeat starting from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default. Cold and cached compile times are medians of --compile-repeats runs (default 3); latest.json stores every run plus the minimum and maximum.

> compile_large, modules_compile, package_compile: compile-only workload(s). They measure how the lsaca build time scales with program size and project layout; they are not executed in the runtime or peak-memory tables.

> modules_compile is a multi-module program built from its crate root file, which loads the sibling module files; package_compile is a package with path dependencies built in package mode (no file argument; the manifest is discovered from the package directory).

## Runtime (median (min), milliseconds; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 37.4 (34.0) | 16.1 (15.6) | 17.2 (16.6) | 27.6 (23.9) | 108.4 (99.4) | 23.0 (20.6) | 43.6 (40.4) | 14.5 (14.0) | 18.0 (16.6) | 192.9 (189.0) | 36.8 (35.4) | 25.9 (25.1) | 36.0 (34.1) | 34.2 (33.8) | 20.1 (19.3) | 49.3 (43.5) | 19.0 (17.4) | 25.2 (23.1) | 15.5 (15.0) | 42.7 (40.6) | 234.4 (220.9) | 227.5 (224.0) |
| C (gcc -O2) | 30.5 (27.7) | 14.0 (12.8) | 11.6 (11.0) | 22.6 (17.5) | 15.9 (14.4) | 16.4 (15.7) | 30.9 (28.8) | 10.0 (9.2) | 12.2 (11.7) | 23.6 (21.0) | 20.8 (19.7) | 12.9 (12.4) | 30.0 (28.8) | 32.0 (28.7) | 18.1 (17.6) | 28.6 (27.0) | 10.6 (9.8) | 22.0 (20.6) | 13.0 (12.0) | 14.9 (13.3) | 148.8 (144.2) | 246.2 (170.5) |
| Rust (rustc -O) | 25.4 (22.5) | n/a | n/a | 22.7 (18.9) | 74.5 (67.2) | 16.9 (15.8) | 32.5 (30.8) | n/a | 12.1 (11.8) | 24.4 (22.2) | 24.2 (23.4) | 13.2 (12.4) | 28.2 (26.7) | 30.1 (28.8) | 20.5 (19.1) | 53.7 (52.1) | n/a | 19.6 (19.1) | 13.3 (12.7) | 15.1 (13.8) | 200.7 (188.9) | 256.8 (249.5) |
| Python 3 | 1273.7 (1145.5) | n/a | n/a | 1138.7 (1057.9) | 450.6 (411.9) | 549.2 (503.9) | 761.8 (629.9) | n/a | 1306.9 (1165.6) | 105.5 (98.8) | 119.8 (107.8) | 64.3 (60.7) | 546.6 (523.6) | 2523.5 (2367.7) | 231.5 (227.3) | 115.5 (109.9) | n/a | 364.1 (341.7) | 41.8 (39.4) | 240.6 (226.7) | 2118.3 (1887.5) | 1031.6 (994.4) |
| Node.js | 307.2 (262.5) | n/a | n/a | 177.0 (148.1) | 266.1 (235.5) | 268.1 (231.4) | 224.3 (212.7) | n/a | 139.2 (136.6) | 155.6 (144.6) | 149.2 (143.5) | 148.2 (134.0) | 186.7 (156.0) | 166.5 (160.0) | 156.9 (151.0) | 174.9 (152.3) | n/a | 152.0 (149.3) | 128.9 (124.5) | 174.8 (161.7) | 665.8 (639.1) | 373.4 (357.8) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 5.1 | 5.1 | 5.1 | 5.1 | 11.7 | 20.1 | 7.9 | 5.2 | 5.5 | 53.1 | 17.1 | 10.6 | 20.1 | 5.1 | 12.5 | 5.5 | 5.1 | 13.9 | 5.2 | 6.4 | 56.6 | 30.8 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.0 | 4.4 | 7.7 | 7.7 | 4.1 | 18.8 | 4.0 | 11.2 | 4.2 | 4.1 | 12.6 | 4.0 | 4.3 | 52.5 | 19.7 |
| Rust (rustc -O) | 4.2 | n/a | n/a | 4.2 | 7.4 | 18.9 | 5.2 | n/a | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 | 11.3 | 4.4 | n/a | 12.7 | 4.1 | 4.7 | 55.9 | 19.8 |
| Python 3 | 11.0 | n/a | n/a | 10.9 | 74.5 | 142.8 | 20.4 | n/a | 13.8 | 21.0 | 21.1 | 17.1 | 26.2 | 11.0 | 49.2 | 11.4 | n/a | 54.2 | 11.1 | 25.6 | 113.5 | 38.0 |
| Node.js | 42.9 | n/a | n/a | 44.9 | 88.4 | 121.3 | 51.6 | n/a | 47.1 | 56.4 | 56.5 | 50.9 | 59.0 | 44.6 | 58.0 | 46.6 | n/a | 72.3 | 43.1 | 69.2 | 102.7 | 90.6 |

## Runtime by data scale (median (min), milliseconds; lower is better)

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

> The medium scale reuses the primary measurement above (default arguments); the small and large scales pass explicit arguments, listed under Data scales below.

## Artifact sizes (bytes)

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

