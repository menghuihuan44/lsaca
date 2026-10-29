# lsaca cross-language benchmarks

Generated: 2026-10-29T21:36:59+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 1.5.0 (target\bench-tools\lsaca.exe)
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
| fib | 0.781 | 0.013 | n/a | 0.216 | 0.207 | n/a | n/a |
| loop_sum | 0.775 | 0.014 | n/a | 0.219 | 0.219 | n/a | n/a |
| string_build | 0.741 | 0.013 | n/a | 0.194 | 0.223 | n/a | n/a |
| vec_memory | 1.047 | 0.020 | n/a | 0.295 | 0.254 | n/a | n/a |
| sort_i64 | 0.837 | 0.015 | n/a | 0.225 | 0.285 | n/a | n/a |
| matrix_multiply | 0.893 | 0.019 | n/a | 0.262 | 0.250 | n/a | n/a |
| binary_trees | 0.747 | 0.014 | n/a | 0.245 | 0.204 | n/a | n/a |
| binary_trees_refs | 0.801 | 0.019 | n/a | 0.284 | 0.234 | n/a | n/a |
| string_split | 0.776 | 0.013 | n/a | 0.207 | 0.287 | n/a | n/a |
| sieve | 0.827 | 0.014 | n/a | 0.202 | 0.215 | n/a | n/a |
| mandelbrot | 0.877 | 0.016 | n/a | 0.213 | 0.245 | n/a | n/a |
| iterator_pipeline | 0.786 | 0.015 | n/a | 0.215 | 0.246 | n/a | n/a |
| hashmap_wordcount | 0.866 | 0.017 | n/a | 0.237 | 0.385 | n/a | n/a |
| generic_stack | 0.897 | 0.015 | n/a | 0.255 | 0.299 | n/a | n/a |
| fn_pipeline | 0.864 | 0.020 | n/a | 0.261 | 0.252 | n/a | n/a |
| compile_large (compile-only) | 3.144 | 0.227 | n/a | n/a | n/a | n/a | n/a |

> lsaca cold build: one default `lsaca build -r` from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default.

> compile_large: compile-only workload(s). They are generated sources used to measure how the lsaca build time scales with program size; they are not executed in the runtime or peak-memory tables.

## Runtime (median (min), milliseconds; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 20.4 (18.8) | 8.1 (7.6) | 27.0 (24.8) | 18.6 (15.5) | 28.0 (26.7) | 12.8 (11.6) | 534.4 (519.3) | 34.1 (30.9) | 15.7 (14.4) | 24.0 (23.1) | 26.4 (23.8) | 27.8 (27.5) | 46.4 (42.1) | 14.2 (13.6) | 9.9 (9.0) |
| C (gcc -O2) | 21.5 (19.2) | 9.2 (7.6) | 10.0 (9.7) | 15.9 (15.3) | 29.8 (25.6) | 12.7 (12.2) | 17.7 (17.3) | 15.1 (14.4) | 11.9 (10.7) | 22.9 (22.4) | 27.6 (26.9) | 12.0 (11.0) | 24.3 (21.9) | 17.4 (16.4) | 11.0 (10.7) |
| Rust (rustc -O) | 10.2 (9.5) | 10.0 (9.4) | 56.2 (54.0) | 14.5 (14.0) | 24.7 (23.7) | 9.0 (8.9) | 19.4 (18.5) | 17.7 (17.0) | 11.4 (11.1) | 23.9 (22.9) | 26.9 (25.6) | 14.5 (11.9) | 43.4 (39.8) | 16.1 (14.8) | 9.0 (8.5) |
| Python 3 | 810.6 (749.6) | 805.6 (726.3) | 370.6 (326.7) | 417.4 (397.4) | 574.9 (549.3) | 1179.3 (1081.7) | 97.4 (92.2) | 83.4 (79.0) | 55.9 (52.8) | 474.3 (439.7) | 2138.4 (1997.9) | 213.1 (211.2) | 101.2 (88.1) | 313.0 (295.9) | 35.8 (33.4) |
| Node.js | 190.0 (176.2) | 101.2 (96.3) | 250.5 (223.6) | 195.8 (178.9) | 142.7 (129.3) | 106.2 (103.2) | 110.9 (105.8) | 107.5 (106.4) | 117.2 (113.6) | 157.2 (137.7) | 119.8 (117.6) | 138.1 (130.5) | 118.3 (111.0) | 126.4 (123.1) | 113.7 (107.7) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 18.8 | 6.6 | 4.4 | 101.2 | 19.9 | 11.1 | 18.9 | 4.0 | 11.2 | 4.3 | 12.6 | 4.1 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.6 | 7.7 | 7.7 | 4.1 | 18.9 | 4.0 | 11.2 | 4.3 | 12.6 | 4.0 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 | 11.2 | 4.4 | 12.7 | 4.2 |
| Python 3 | 11.0 | 10.9 | 73.6 | 142.8 | 21.1 | 13.6 | 21.0 | 21.0 | 16.9 | 26.2 | 11.0 | 49.3 | 11.4 | 54.3 | 11.1 |
| Node.js | 43.0 | 45.7 | 89.3 | 121.3 | 50.2 | 47.3 | 56.0 | 56.1 | 51.1 | 60.3 | 44.7 | 58.1 | 46.4 | 72.4 | 40.2 |

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
- compile_large: generated 4,511-line lsaca source (450 functions) used to measure how the lsaca build time scales with program size; compile-only (expected stdout checksum=14655037, not measured by this harness)

