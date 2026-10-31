# lsaca cross-language benchmarks

Generated: 2026-10-31T23:24:44+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 1.6.0 (target\bench-tools\lsaca.exe)
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
| fib | 1.121 | 0.018 | n/a | 0.257 | 0.230 | n/a | n/a |
| loop_sum | 1.190 | 0.021 | n/a | 0.228 | 0.252 | n/a | n/a |
| string_build | 0.917 | 0.015 | n/a | 0.209 | 0.248 | n/a | n/a |
| vec_memory | 0.925 | 0.014 | n/a | 0.206 | 0.226 | n/a | n/a |
| sort_i64 | 1.219 | 0.020 | n/a | 0.292 | 0.324 | n/a | n/a |
| matrix_multiply | 1.105 | 0.018 | n/a | 0.263 | 0.294 | n/a | n/a |
| binary_trees | 0.931 | 0.015 | n/a | 0.238 | 0.209 | n/a | n/a |
| binary_trees_refs | 0.915 | 0.015 | n/a | 0.247 | 0.207 | n/a | n/a |
| string_split | 0.907 | 0.014 | n/a | 0.209 | 0.274 | n/a | n/a |
| sieve | 1.039 | 0.020 | n/a | 0.235 | 0.238 | n/a | n/a |
| mandelbrot | 1.106 | 0.016 | n/a | 0.206 | 0.210 | n/a | n/a |
| iterator_pipeline | 0.985 | 0.016 | n/a | 0.218 | 0.296 | n/a | n/a |
| hashmap_wordcount | 1.032 | 0.015 | n/a | 0.254 | 0.422 | n/a | n/a |
| generic_stack | 1.031 | 0.016 | n/a | 0.241 | 0.276 | n/a | n/a |
| fn_pipeline | 0.968 | 0.016 | n/a | 0.211 | 0.239 | n/a | n/a |
| string_unicode | 1.006 | 0.017 | n/a | 0.284 | 0.354 | n/a | n/a |
| hashmap_i64 | 0.931 | 0.017 | n/a | 0.249 | 0.448 | n/a | n/a |
| hashmap_string | 0.979 | 0.016 | n/a | 0.242 | 0.564 | n/a | n/a |
| compile_large (compile-only) | 1.833 | 0.244 | n/a | n/a | n/a | n/a | n/a |

> lsaca cold build: one default `lsaca build -r` from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default.

> compile_large: compile-only workload(s). They are generated sources used to measure how the lsaca build time scales with program size; they are not executed in the runtime or peak-memory tables.

## Runtime (median (min), milliseconds; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 25.3 (25.0) | 9.6 (9.1) | 28.6 (28.0) | 12.7 (12.3) | 37.0 (32.1) | 12.4 (11.9) | 322.4 (315.9) | 26.8 (24.6) | 16.0 (15.2) | 32.6 (31.1) | 24.9 (23.0) | 25.4 (24.4) | 46.8 (41.8) | 12.8 (12.0) | 7.6 (7.4) | 27.9 (25.9) | 211.3 (202.8) | 241.7 (233.8) |
| C (gcc -O2) | 24.5 (23.1) | 8.9 (8.7) | 11.0 (10.5) | 12.3 (12.1) | 33.8 (32.2) | 12.6 (12.0) | 16.0 (15.1) | 16.1 (15.4) | 10.5 (9.9) | 28.0 (26.5) | 24.3 (23.4) | 12.8 (10.8) | 23.4 (21.8) | 12.8 (12.2) | 7.2 (6.9) | 10.0 (9.7) | 133.0 (131.4) | 165.8 (162.3) |
| Rust (rustc -O) | 12.9 (10.7) | 9.3 (8.3) | 56.4 (53.4) | 13.8 (13.0) | 31.7 (29.9) | 12.1 (11.6) | 17.6 (16.6) | 17.3 (16.6) | 10.7 (10.3) | 26.5 (25.8) | 24.1 (23.7) | 15.3 (13.6) | 43.2 (40.0) | 14.5 (13.5) | 8.1 (7.3) | 11.7 (11.2) | 172.1 (166.6) | 246.8 (241.3) |
| Python 3 | 983.7 (804.8) | 912.2 (817.1) | 347.1 (343.8) | 422.5 (415.0) | 632.3 (609.4) | 1149.7 (1128.2) | 86.1 (82.3) | 85.6 (79.2) | 57.7 (55.4) | 467.1 (453.3) | 2215.3 (2181.2) | 218.5 (205.0) | 104.4 (97.6) | 301.3 (291.0) | 32.5 (31.2) | 222.1 (209.1) | 1722.3 (1688.5) | 995.9 (912.1) |
| Node.js | 213.4 (210.0) | 109.3 (106.7) | 171.8 (162.7) | 227.0 (201.4) | 162.2 (161.0) | 116.5 (114.5) | 113.1 (107.4) | 113.2 (109.9) | 113.0 (108.8) | 142.0 (124.6) | 127.0 (122.9) | 136.7 (133.5) | 125.6 (121.3) | 129.2 (124.1) | 104.4 (102.2) | 158.3 (148.6) | 608.5 (596.3) | 370.4 (343.5) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.1 | 4.1 | 10.5 | 18.8 | 6.6 | 4.4 | 52.3 | 15.8 | 9.3 | 18.9 | 4.1 | 11.2 | 4.2 | 12.6 | 4.1 | 5.4 | 55.5 | 32.3 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.4 | 7.7 | 7.7 | 4.1 | 18.9 | 4.0 | 11.2 | 4.3 | 12.6 | 4.0 | 4.3 | 52.4 | 19.7 |
| Rust (rustc -O) | 4.1 | 4.2 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 | 11.3 | 4.4 | 12.7 | 4.2 | 4.7 | 55.9 | 19.8 |
| Python 3 | 11.0 | 11.0 | 74.1 | 142.8 | 21.4 | 13.6 | 21.1 | 21.0 | 16.8 | 26.2 | 11.0 | 49.3 | 11.4 | 54.2 | 11.1 | 26.0 | 113.4 | 38.0 |
| Node.js | 43.0 | 45.8 | 89.4 | 121.4 | 50.4 | 47.6 | 56.4 | 56.4 | 50.8 | 60.2 | 44.7 | 58.1 | 46.4 | 72.3 | 40.3 | 68.2 | 102.4 | 90.7 |

## Workloads

- fib: recursive fibonacci(30), called 10 times, checksummed; expected stdout checksum=8320400
- loop_sum: integer sum 1..=8,000,000; expected stdout checksum=32000004000000
- string_build: append 1,000,000 numeric tokens (i % 1000) to a string; checksum is length,sum-of-token-values; expected stdout checksum=3890000,499500000
- vec_memory: build a vector of 1,000,000 Point{x=i64, y=i64}, then checksum; expected stdout checksum=1499998500000
- sort_i64: sort 200,000 pseudo-random i64 values with an iterative quicksort written in the language; checksum is first,last,sum; expected stdout checksum=21749,2147481261,214813518297651
- matrix_multiply: multiply two 160x160 i64 matrices with the naive triple loop; checksum is the sum of the result entries; expected stdout checksum=1018924712000
- binary_trees: build a perfect binary tree of depth 16 (131,071 nodes) and sum all payloads; expected stdout checksum=2147450880
- binary_trees_refs: the same perfect binary tree of depth 16 (131,071 nodes) as binary_trees, but summed through borrowed references (&Option<Box<Node>>) instead of by value, so the traversal does not copy the tree; checksum matches binary_trees; expected stdout checksum=2147450880
- string_split: build a 194,500-byte comma-separated numeric string, split it on commas, parse every token and sum; checksum is len,count,sum; expected stdout checksum=194500,50000,24975000
- sieve: sieve of Eratosthenes: count the primes below 2,000,000; expected stdout checksum=148933
- mandelbrot: 600x400 Mandelbrot escape-time grid, at most 100 iterations per pixel; checksum is the sum of iteration counts; expected stdout checksum=6282265
- iterator_pipeline: run 1,000,000 i64 values through the lazy chain filter(even).map(x * x).fold(0, add) and print the folded sum; measures iterator adapter overhead against a hand-written loop; expected stdout checksum=166666166667000000
- hashmap_wordcount: count 200,000 deterministic words drawn from a 4,096-word alphabet in a hash map, then fold the squared counts; checksum is the sum of squared counts, the number of distinct keys and the highest frequency (order-independent); expected stdout checksum=9766208,4096,49
- generic_stack: the same generic stack instantiated for i64 and for String: push/pop 1,000,000 integers and 1,000 strings; checksum is integer sum and total string byte length; expected stdout checksum=499999500000,5900
- fn_pipeline: rule records that carry a label String and a fn(i64) -> i64 value in one struct, applied 1,000 times each plus a clone of the first rule; checksum is the result sum and total label byte length; expected stdout checksum=53637,22
- string_unicode: build a 120,000-scalar mixed ASCII/Unicode string one char at a time, fold every scalar from iteration and from 1,000 random char_at accesses, apply full-Unicode to_uppercase, slice on character boundaries, push/pop and extend; checksum is char count, byte length and the folded scalar checksum; expected stdout checksum=3357,6921,368454760
- hashmap_i64: 1,000,000 integer-key inserts, 1,000,000 present lookups, 1,000,000 absent lookups and 100,000 deletions in a hash map; checksum is the folded value sum, the final length and the number of successful lookups; expected stdout checksum=821060469,900000,1000000
- hashmap_string: 200,000 distinct generated String keys inserted into a hash map, then 200,000 present lookups, 200,000 absent lookups (folding the bytes of each absent key) and 20,000 deletions; checksum is the folded value sum, the final length and the number of successful lookups; expected stdout checksum=794025725,180000,200000
- compile_large: generated 4,511-line lsaca source (450 functions) used to measure how the lsaca build time scales with program size; compile-only (expected stdout checksum=14655037, not measured by this harness)

