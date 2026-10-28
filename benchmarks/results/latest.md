# lsaca cross-language benchmarks

Generated: 2026-10-28T22:50:42+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 1.4.0 (target\bench-tools\lsaca.exe)
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
| fib | 1.206 | 0.028 | n/a | 0.420 | 0.360 | n/a | n/a |
| loop_sum | 1.199 | 0.025 | n/a | 0.266 | 0.307 | n/a | n/a |
| string_build | 1.031 | 0.020 | n/a | 0.296 | 0.339 | n/a | n/a |
| vec_memory | 1.147 | 0.025 | n/a | 0.332 | 0.369 | n/a | n/a |
| sort_i64 | 1.223 | 0.027 | n/a | 0.385 | 0.409 | n/a | n/a |
| matrix_multiply | 2.823 | 0.022 | n/a | 0.290 | 0.332 | n/a | n/a |
| binary_trees | 1.066 | 0.022 | n/a | 0.367 | 0.322 | n/a | n/a |
| binary_trees_refs | 1.065 | 0.022 | n/a | 0.412 | 0.365 | n/a | n/a |
| string_split | 1.232 | 0.024 | n/a | 0.331 | 0.417 | n/a | n/a |
| sieve | 1.321 | 0.026 | n/a | 0.308 | 0.325 | n/a | n/a |
| mandelbrot | 1.253 | 0.036 | n/a | 0.285 | 0.359 | n/a | n/a |
| compile_large (compile-only) | 4.031 | 0.324 | n/a | n/a | n/a | n/a | n/a |

> lsaca cold build: one default `lsaca build -r` from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default.

> compile_large: compile-only workload(s). They are generated sources used to measure how the lsaca build time scales with program size; they are not executed in the runtime or peak-memory tables.

## Runtime (median (min), milliseconds; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 35.8 (33.9) | 12.7 (12.2) | 39.4 (38.2) | 23.4 (19.2) | 41.4 (39.1) | 16.1 (15.8) | 677.3 (662.8) | 35.8 (35.6) | 31.7 (22.5) | 36.6 (33.1) | 34.3 (33.6) |
| C (gcc -O2) | 43.8 (34.6) | 12.3 (12.0) | 16.7 (15.5) | 22.4 (19.1) | 37.2 (33.4) | 16.6 (15.6) | 25.1 (23.6) | 26.9 (24.9) | 19.5 (18.5) | 34.5 (34.1) | 34.9 (33.2) |
| Rust (rustc -O) | 24.9 (21.1) | 19.0 (14.3) | 96.1 (89.5) | 23.9 (21.6) | 35.6 (35.0) | 15.6 (15.0) | 24.6 (23.9) | 26.3 (25.2) | 17.2 (16.5) | 36.2 (34.9) | 38.3 (34.9) |
| Python 3 | 1180.9 (1116.6) | 1147.1 (1072.2) | 471.4 (466.0) | 636.0 (573.2) | 803.3 (729.1) | 1689.4 (1535.4) | 111.9 (110.9) | 146.2 (124.2) | 80.3 (77.6) | 599.0 (581.5) | 2869.9 (2773.7) |
| Node.js | 346.6 (329.6) | 165.6 (154.9) | 236.6 (225.7) | 261.1 (255.8) | 201.1 (189.3) | 161.7 (153.4) | 154.9 (152.4) | 184.1 (175.8) | 169.5 (159.2) | 201.8 (177.9) | 162.2 (156.1) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.1 | 10.4 | 18.8 | 6.6 | 4.4 | 101.1 | 19.9 | 11.1 | 18.9 | 4.0 |
| C (gcc -O2) | 4.0 | 4.0 | 7.4 | 18.8 | 5.1 | 4.4 | 7.7 | 7.7 | 4.0 | 18.9 | 4.1 |
| Rust (rustc -O) | 4.2 | 4.2 | 7.4 | 18.9 | 5.2 | 4.3 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 |
| Python 3 | 11.1 | 11.0 | 73.8 | 142.2 | 21.4 | 13.7 | 21.1 | 21.1 | 17.0 | 26.3 | 11.1 |
| Node.js | 42.8 | 45.7 | 89.6 | 121.1 | 50.9 | 46.9 | 56.2 | 56.2 | 50.6 | 60.0 | 44.9 |

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

