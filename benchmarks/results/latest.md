# lsaca cross-language benchmarks

Generated: 2026-10-25T12:52:17+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 1.2.0 (target\bench-tools\lsaca.exe)
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
| fib | 0.988 | 0.027 | n/a | 0.367 | 0.295 | n/a | n/a |
| loop_sum | 1.050 | 0.022 | n/a | 0.246 | 0.277 | n/a | n/a |
| string_build | 1.053 | 0.023 | n/a | 0.265 | 0.291 | n/a | n/a |
| vec_memory | 0.968 | 0.019 | n/a | 0.269 | 0.303 | n/a | n/a |
| sort_i64 | 1.063 | 0.024 | n/a | 0.398 | 0.417 | n/a | n/a |
| matrix_multiply | 1.297 | 0.031 | n/a | 0.363 | 0.445 | n/a | n/a |
| binary_trees | 1.151 | 0.020 | n/a | 0.400 | 0.353 | n/a | n/a |
| string_split | 1.203 | 0.020 | n/a | 0.370 | 0.450 | n/a | n/a |
| sieve | 1.206 | 0.025 | n/a | 0.388 | 0.414 | n/a | n/a |
| mandelbrot | 1.365 | 0.029 | n/a | 0.386 | 0.362 | n/a | n/a |
| compile_large (compile-only) | 4.668 | 0.165 | n/a | n/a | n/a | n/a | n/a |

> lsaca cold build: one default `lsaca build -r` from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default.

> compile_large: compile-only workload(s). They are generated sources used to measure how the lsaca build time scales with program size; they are not executed in the runtime or peak-memory tables.

## Runtime (median (min), milliseconds; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 34.9 (31.7) | 11.7 (10.7) | 33.9 (33.0) | 16.8 (16.3) | 35.7 (35.3) | 26.4 (19.0) | 1261.4 (1146.8) | 26.5 (25.6) | 50.6 (42.2) | 40.6 (38.1) |
| C (gcc -O2) | 30.6 (28.9) | 10.9 (10.3) | 15.2 (14.7) | 17.1 (16.8) | 37.4 (34.8) | 18.4 (16.4) | 27.8 (22.9) | 20.2 (17.6) | 50.1 (45.7) | 40.7 (38.9) |
| Rust (rustc -O) | 14.5 (14.0) | 14.5 (11.4) | 68.3 (65.0) | 18.4 (17.8) | 33.2 (32.6) | 24.2 (22.8) | 28.6 (24.4) | 20.7 (17.2) | 46.7 (42.0) | 42.1 (38.2) |
| Python 3 | 1056.4 (1015.3) | 1024.0 (1010.3) | 423.7 (414.9) | 576.8 (544.9) | 864.4 (709.4) | 1887.4 (1512.2) | 132.8 (114.3) | 92.9 (87.5) | 687.4 (623.1) | 3366.2 (3311.7) |
| Node.js | 270.9 (238.4) | 143.3 (140.4) | 210.2 (202.5) | 260.0 (242.0) | 246.0 (208.4) | 221.4 (193.8) | 197.9 (161.1) | 183.1 (155.7) | 222.2 (217.5) | 206.9 (184.5) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 18.8 | 6.6 | 4.4 | 149.7 | 11.1 | 18.8 | 4.0 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.3 | 7.6 | 4.0 | 18.8 | 4.0 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 4.4 | 18.9 | 4.1 |
| Python 3 | 11.0 | 10.9 | 74.0 | 143.8 | 20.6 | 13.6 | 21.0 | 17.2 | 26.2 | 11.0 |
| Node.js | 42.8 | 45.7 | 89.3 | 117.2 | 50.6 | 46.8 | 56.1 | 50.6 | 59.9 | 44.6 |

## Workloads

- fib: recursive fibonacci(30), called 10 times, checksummed; expected stdout checksum=8320400
- loop_sum: integer sum 1..=8,000,000; expected stdout checksum=32000004000000
- string_build: append 1,000,000 numeric tokens (i % 1000) to a string; checksum is length,sum-of-token-values; expected stdout checksum=3890000,499500000
- vec_memory: build a vector of 1,000,000 Point{x=i64, y=i64}, then checksum; expected stdout checksum=1499998500000
- sort_i64: sort 200,000 pseudo-random i64 values with an iterative quicksort written in the language; checksum is first,last,sum; expected stdout checksum=21749,2147481261,214813518297651
- matrix_multiply: multiply two 160x160 i64 matrices with the naive triple loop; checksum is the sum of the result entries; expected stdout checksum=1018924712000
- binary_trees: build a perfect binary tree of depth 16 (131,071 nodes) and sum all payloads; expected stdout checksum=2147450880
- string_split: build a 194,500-byte comma-separated numeric string, split it on commas, parse every token and sum; checksum is len,count,sum; expected stdout checksum=194500,50000,24975000
- sieve: sieve of Eratosthenes: count the primes below 2,000,000; expected stdout checksum=148933
- mandelbrot: 600x400 Mandelbrot escape-time grid, at most 100 iterations per pixel; checksum is the sum of iteration counts; expected stdout checksum=6282265
- compile_large: generated 4,511-line lsaca source (450 functions) used to measure how the lsaca build time scales with program size; compile-only (expected stdout checksum=14655037, not measured by this harness)

## Warnings

- The primary cold-build sample for string_build/lsaca was 32.1 s because an unrelated background build was running on the same machine at that moment; that single cell was re-measured with the same harness settings at 2026-10-25T13:01:46+08:00 (1.053 s, used here). All other numbers are from the primary run at 2026-10-25T12:52:17+08:00.

