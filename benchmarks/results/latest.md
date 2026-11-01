# lsaca cross-language benchmarks

Generated: 2026-11-01T13:25:52+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 1.7.0 (target\bench-tools\lsaca.exe)
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
| fib | 1.170 | 0.016 | n/a | 0.262 | 0.216 | n/a | n/a |
| loop_sum | 1.136 | 0.016 | n/a | 0.212 | 0.205 | n/a | n/a |
| string_build | 1.033 | 0.017 | n/a | 0.230 | 0.247 | n/a | n/a |
| vec_memory | 1.022 | 0.016 | n/a | 0.224 | 0.254 | n/a | n/a |
| sort_i64 | 1.078 | 0.020 | n/a | 0.242 | 0.270 | n/a | n/a |
| matrix_multiply | 1.056 | 0.016 | n/a | 0.227 | 0.250 | n/a | n/a |
| binary_trees | 0.992 | 0.016 | n/a | 0.249 | 0.222 | n/a | n/a |
| binary_trees_refs | 1.008 | 0.016 | n/a | 0.262 | 0.221 | n/a | n/a |
| string_split | 1.025 | 0.016 | n/a | 0.224 | 0.291 | n/a | n/a |
| sieve | 1.017 | 0.016 | n/a | 0.211 | 0.228 | n/a | n/a |
| mandelbrot | 1.003 | 0.017 | n/a | 0.219 | 0.214 | n/a | n/a |
| iterator_pipeline | 1.002 | 0.017 | n/a | 0.221 | 0.247 | n/a | n/a |
| hashmap_wordcount | 1.057 | 0.016 | n/a | 0.241 | 0.408 | n/a | n/a |
| generic_stack | 1.007 | 0.017 | n/a | 0.234 | 0.260 | n/a | n/a |
| fn_pipeline | 0.997 | 0.018 | n/a | 0.223 | 0.233 | n/a | n/a |
| string_unicode | 0.990 | 0.017 | n/a | 0.280 | 0.367 | n/a | n/a |
| hashmap_i64 | 0.983 | 0.016 | n/a | 0.265 | 0.456 | n/a | n/a |
| hashmap_string | 1.025 | 0.016 | n/a | 0.268 | 0.455 | n/a | n/a |
| compile_large (compile-only) | 1.837 | 0.238 | n/a | n/a | n/a | n/a | n/a |
| modules_compile (compile-only) | 2.379 | 0.199 | n/a | n/a | n/a | n/a | n/a |
| package_compile (compile-only) | 1.482 | 0.042 | n/a | n/a | n/a | n/a | n/a |

> lsaca cold build: one default `lsaca build -r` from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default.

> compile_large, modules_compile, package_compile: compile-only workload(s). They measure how the lsaca build time scales with program size and project layout; they are not executed in the runtime or peak-memory tables.

> modules_compile is a multi-module program built from its crate root file, which loads the sibling module files; package_compile is a package with path dependencies built in package mode (no file argument; the manifest is discovered from the package directory).

## Runtime (median (min), milliseconds; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 26.1 (23.7) | 8.0 (7.2) | 30.0 (26.7) | 13.5 (12.1) | 30.5 (30.2) | 9.9 (9.5) | 331.3 (315.6) | 25.9 (25.0) | 15.9 (14.8) | 26.1 (24.2) | 23.0 (22.1) | 27.7 (25.3) | 44.2 (42.3) | 12.9 (12.6) | 7.6 (7.3) | 27.9 (26.6) | 213.0 (206.8) | 243.7 (237.1) |
| C (gcc -O2) | 26.4 (23.6) | 7.8 (7.6) | 11.5 (10.9) | 14.4 (13.1) | 26.6 (25.2) | 9.6 (9.3) | 16.3 (15.3) | 16.0 (15.5) | 11.4 (11.2) | 23.4 (22.8) | 23.9 (22.6) | 10.7 (10.4) | 22.2 (21.6) | 12.5 (12.2) | 7.5 (7.3) | 10.9 (10.2) | 135.1 (133.3) | 164.8 (153.4) |
| Rust (rustc -O) | 10.7 (10.6) | 8.0 (7.5) | 61.5 (58.8) | 14.1 (13.4) | 26.3 (26.3) | 10.1 (9.8) | 18.0 (17.3) | 17.3 (16.5) | 11.4 (10.5) | 24.0 (23.7) | 24.6 (23.3) | 10.8 (10.2) | 42.3 (37.3) | 14.0 (12.7) | 7.8 (7.5) | 12.0 (11.2) | 168.5 (166.5) | 213.1 (207.3) |
| Python 3 | 916.7 (874.6) | 884.4 (854.4) | 370.7 (359.0) | 457.7 (446.4) | 547.9 (538.8) | 1172.1 (1135.9) | 90.7 (86.1) | 90.4 (88.4) | 60.8 (58.3) | 491.9 (481.5) | 2178.8 (2158.9) | 197.8 (192.9) | 96.7 (93.2) | 290.1 (284.9) | 35.4 (32.1) | 226.4 (221.2) | 1813.8 (1791.2) | 913.0 (903.4) |
| Node.js | 209.5 (199.9) | 116.5 (112.0) | 185.5 (181.1) | 200.8 (190.1) | 153.3 (147.2) | 119.0 (110.3) | 121.1 (114.8) | 120.6 (115.4) | 122.2 (114.0) | 133.3 (127.6) | 128.7 (116.7) | 133.7 (126.2) | 124.2 (119.1) | 130.7 (123.2) | 107.8 (105.3) | 164.8 (158.6) | 591.6 (584.0) | 335.0 (322.5) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.1 | 4.0 | 10.4 | 18.8 | 6.6 | 4.4 | 52.2 | 15.7 | 9.3 | 18.9 | 4.0 | 11.2 | 4.2 | 12.6 | 4.1 | 5.3 | 55.5 | 32.3 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.4 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 | 11.2 | 4.2 | 12.6 | 4.0 | 4.3 | 52.4 | 19.7 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.1 | 11.2 | 4.7 | 12.6 | 4.1 | 4.7 | 55.9 | 19.8 |
| Python 3 | 11.0 | 10.9 | 73.7 | 142.8 | 19.5 | 13.6 | 21.0 | 21.0 | 16.8 | 26.2 | 11.0 | 49.2 | 11.4 | 54.2 | 11.1 | 25.6 | 113.4 | 38.1 |
| Node.js | 43.0 | 45.7 | 89.6 | 121.5 | 50.3 | 47.1 | 56.0 | 56.3 | 51.0 | 60.1 | 44.6 | 58.1 | 46.5 | 72.2 | 39.9 | 68.2 | 102.6 | 90.7 |

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
- modules_compile: multi-module program: one crate root (main.ls) plus eight sibling module files, about 3,600 lines in total, with structs, enums, generics, trait impls, strings, vectors and hash maps; used to measure the multi-file build path; compile-only (expected stdout checksum=547598677, not measured by this harness)
- package_compile: package with path dependencies: the binary package compile_app plus the library packages util_a and util_b (util_b depends on util_a), about 1,090 lines spread over thirteen files, used to measure package-mode builds; compile-only (expected stdout checksum=417367897, not measured by this harness)

