# lsaca cross-language benchmarks

Generated: 2026-11-27T09:03:48+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 2.3.0 (target\bench-tools\lsaca.exe)
- lsaca build backend: direct C compiler back end (default; --cmake available)
- gcc: gcc.EXE (Rev6, Built by MSYS2 project) 16.2.0
- rustc: rustc 1.99.0 (b940084d7 2026-09-28)
- Python: Python 3.14.10
- Node.js: v26.13.0
- cmake: cmake version 4.1.0
- ninja: 1.13.2
- lsaca build mode: official

## Compile time (seconds, median of 3 runs; lower is better)

| program | lsaca cold build (default back end) | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
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

> lsaca cold build: the default `lsaca build -r`, repeated --compile-repeats times (default 3), each repeat starting from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default. Cold and cached compile times are medians of --compile-repeats runs (default 3); latest.json stores every run plus the minimum and maximum.

> compile_large, modules_compile, package_compile: compile-only workload(s). They measure how the lsaca build time scales with program size and project layout; they are not executed in the runtime or peak-memory tables.

> modules_compile is a multi-module program built from its crate root file, which loads the sibling module files; package_compile is a package with path dependencies built in package mode (no file argument; the manifest is discovered from the package directory).

## Runtime (median (min), milliseconds; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 33.1 (30.5) | 15.4 (13.9) | 17.1 (14.1) | 17.5 (16.3) | 85.5 (76.9) | 18.4 (17.6) | 39.0 (36.8) | 18.3 (16.9) | 18.7 (18.2) | 209.1 (202.7) | 38.8 (34.5) | 34.1 (28.5) | 57.7 (47.4) | 38.9 (34.6) | 18.5 (17.6) | 43.3 (38.5) | 18.2 (16.5) | 21.2 (20.1) | 13.7 (13.1) | 41.5 (38.5) | 309.7 (283.0) | 300.2 (252.3) |
| C (gcc -O2) | 33.3 (31.3) | 12.2 (11.2) | 11.3 (10.5) | 12.4 (11.7) | 13.7 (13.0) | 15.6 (14.1) | 35.5 (34.9) | 13.6 (11.1) | 15.5 (15.1) | 16.7 (16.2) | 21.0 (20.2) | 22.0 (17.7) | 38.4 (34.5) | 34.9 (31.4) | 17.3 (16.6) | 32.9 (31.1) | 13.6 (12.4) | 25.1 (21.8) | 13.0 (12.2) | 18.2 (17.3) | 187.4 (181.4) | 230.8 (219.9) |
| Rust (rustc -O) | 20.7 (19.2) | n/a | n/a | 12.9 (11.7) | 56.5 (55.4) | 17.8 (16.7) | 33.9 (31.9) | n/a | 13.5 (12.4) | 24.4 (23.6) | 18.0 (17.4) | 21.1 (18.8) | 32.9 (29.3) | 32.8 (31.6) | 17.1 (17.0) | 51.6 (49.7) | n/a | 19.2 (17.0) | 13.3 (12.8) | 16.1 (15.7) | 209.2 (205.2) | 245.9 (235.8) |
| Python 3 | 1074.2 (975.5) | n/a | n/a | 302.1 (278.6) | 167.2 (163.1) | 264.2 (251.0) | 372.6 (350.4) | n/a | 421.5 (393.3) | 81.3 (76.5) | 83.5 (81.5) | 66.8 (54.7) | 259.9 (251.6) | 726.1 (708.5) | 117.7 (113.1) | 79.4 (74.3) | n/a | 226.8 (212.0) | 38.4 (35.8) | 158.2 (139.3) | 1095.1 (1013.9) | 706.1 (669.5) |
| Node.js | 158.8 (154.0) | n/a | n/a | 95.8 (93.7) | 107.7 (104.2) | 157.2 (145.7) | 118.4 (111.3) | n/a | 88.4 (83.2) | 69.0 (67.3) | 71.4 (70.0) | 88.3 (84.1) | 97.6 (91.4) | 86.9 (84.9) | 96.5 (90.7) | 88.3 (82.0) | n/a | 101.7 (95.2) | 69.2 (66.8) | 134.5 (128.6) | 691.3 (634.2) | 522.5 (478.4) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | fib_iter | fib_memo | loop_sum | string_build | vec_memory | sort_i64 | sort_insertion | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | array_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.5 | 4.5 | 4.5 | 4.5 | 11.0 | 19.3 | 7.1 | 4.5 | 4.6 | 52.3 | 16.2 | 9.8 | 19.3 | 4.5 | 11.6 | 4.7 | 4.5 | 13.0 | 4.5 | 5.6 | 55.8 | 30.0 |
| C (gcc -O2) | 4.0 | 4.0 | 4.0 | 4.0 | 7.3 | 18.8 | 5.0 | 4.0 | 4.3 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.1 | 4.2 | 4.0 | 12.5 | 4.0 | 4.7 | 52.5 | 19.6 |
| Rust (rustc -O) | 4.1 | n/a | n/a | 4.1 | 7.3 | 18.9 | 5.2 | n/a | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.1 | 11.2 | 4.3 | n/a | 12.6 | 4.1 | 4.7 | 55.8 | 19.7 |
| Python 3 | 10.9 | n/a | n/a | 10.9 | 74.4 | 142.7 | 20.2 | n/a | 13.5 | 21.0 | 21.0 | 17.0 | 26.2 | 11.0 | 49.2 | 11.3 | n/a | 54.2 | 11.0 | 25.8 | 113.4 | 38.0 |
| Node.js | 43.1 | n/a | n/a | 44.9 | 93.4 | 128.5 | 51.6 | n/a | 47.2 | 52.5 | 52.6 | 53.5 | 59.5 | 45.3 | 56.0 | 46.7 | n/a | 74.7 | 40.3 | 65.0 | 116.7 | 91.4 |

## Runtime by data scale (median (min), milliseconds; lower is better)

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
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

> The medium scale reuses the primary measurement above (default arguments); the small and large scales pass explicit arguments, listed under Data scales below.

## Artifact sizes (bytes)

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | ---: | ---: | ---: | ---: |
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

