# lsaca cross-language benchmarks

Generated: 2026-10-27T23:19:55+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 1.3.0 (target\bench-tools\lsaca.exe)
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
| fib | 1.545 | 0.031 | n/a | 0.393 | 1.260 | n/a | n/a |
| loop_sum | 1.227 | 0.022 | n/a | 0.303 | 0.933 | n/a | n/a |
| string_build | 1.390 | 0.024 | n/a | 0.339 | 1.414 | n/a | n/a |
| vec_memory | 1.509 | 0.032 | n/a | 0.502 | 1.652 | n/a | n/a |
| sort_i64 | 1.718 | 0.045 | n/a | 0.491 | 1.115 | n/a | n/a |
| matrix_multiply | 1.163 | 0.023 | n/a | 0.343 | 0.989 | n/a | n/a |
| binary_trees | 1.247 | 0.036 | n/a | 0.365 | 0.942 | n/a | n/a |
| binary_trees_refs | 1.151 | 0.023 | n/a | 0.363 | 0.934 | n/a | n/a |
| string_split | 1.186 | 0.031 | n/a | 0.361 | 1.047 | n/a | n/a |
| sieve | 1.163 | 0.026 | n/a | 0.305 | 1.334 | n/a | n/a |
| mandelbrot | 1.368 | 0.029 | n/a | 0.427 | 1.571 | n/a | n/a |
| compile_large (compile-only) | 3.972 | 0.308 | n/a | n/a | n/a | n/a | n/a |

> lsaca cold build: one default `lsaca build -r` from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default.

> compile_large: compile-only workload(s). They are generated sources used to measure how the lsaca build time scales with program size; they are not executed in the runtime or peak-memory tables.

## Runtime (median (min), milliseconds; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 44.2 (38.3) | 13.9 (12.2) | 45.2 (42.4) | 30.7 (27.4) | 63.1 (61.2) | 17.0 (16.5) | 1216.7 (1189.9) | 223.2 (215.5) | 32.2 (28.9) | 39.4 (38.2) | 49.2 (47.2) |
| C (gcc -O2) | 35.7 (33.5) | 14.2 (12.0) | 27.0 (25.2) | 36.4 (30.5) | 50.2 (46.9) | 17.1 (15.8) | 25.8 (23.9) | 24.4 (22.9) | 19.3 (18.1) | 41.5 (37.7) | 63.9 (59.7) |
| Rust (rustc -O) | 23.2 (20.8) | 14.2 (13.3) | 146.4 (127.4) | 34.8 (30.1) | 43.6 (42.5) | 16.2 (15.5) | 26.5 (24.9) | 25.8 (24.6) | 18.6 (17.7) | 38.9 (36.8) | 59.1 (50.8) |
| Python 3 | 1163.1 (1112.7) | 1511.7 (1168.6) | 704.8 (615.6) | 867.7 (773.7) | 756.8 (734.5) | 1538.4 (1518.3) | 117.7 (112.5) | 117.9 (114.2) | 83.8 (78.6) | 772.5 (674.1) | 3423.3 (2729.5) |
| Node.js | 344.3 (281.9) | 214.8 (190.1) | 388.1 (307.8) | 349.0 (326.9) | 202.6 (197.2) | 172.3 (163.5) | 173.6 (164.6) | 179.7 (169.0) | 171.7 (166.9) | 249.6 (230.3) | 192.6 (154.7) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 18.8 | 6.6 | 4.4 | 150.1 | 35.8 | 11.1 | 18.8 | 4.0 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.4 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 7.7 | 4.1 | 18.9 | 4.1 |
| Python 3 | 11.0 | 11.0 | 73.6 | 142.8 | 21.8 | 13.6 | 21.0 | 21.0 | 16.8 | 26.2 | 11.0 |
| Node.js | 43.0 | 44.5 | 89.5 | 121.1 | 51.0 | 46.9 | 56.1 | 56.2 | 50.6 | 59.3 | 44.1 |

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
- compile_large: generated 4,511-line lsaca source (450 functions) used to measure how the lsaca build time scales with program size; compile-only (expected stdout checksum=14655037, not measured by this harness)

