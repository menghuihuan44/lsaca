# lsaca cross-language benchmarks

Generated: 2026-10-24T21:57:52+08:00  
Harness: python scripts/bench.py (see benchmarks/README.md)

## Environment

- OS: Windows-11-10.0.26300-SP0
- CPU: 12th Gen Intel(R) Core(TM) i7-1260P (16 logical CPUs)
- RAM: 15.63 GiB
- lsaca: lsaca 1.1.0 (target\bench-tools\lsaca.exe)
- gcc: gcc.EXE (Rev14, Built by MSYS2 project) 15.2.0
- rustc: rustc 1.99.0 (b940084d7 2026-09-28)
- Python: Python 3.14.4
- Node.js: v24.14.1
- cmake: cmake version 4.1.0
- ninja: 1.13.2
- lsaca build mode: official

## Compile time (seconds, median of 3 runs; lower is better)

| program | lsaca cold build | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| fib | 3.270 | 0.019 | n/a | 0.305 | 0.274 | n/a | n/a |
| loop_sum | 2.874 | 0.018 | n/a | 0.248 | 0.257 | n/a | n/a |
| string_build | 2.787 | 0.017 | n/a | 0.261 | 0.305 | n/a | n/a |
| vec_memory | 2.917 | 0.018 | n/a | 0.260 | 0.356 | n/a | n/a |

## Runtime (median (min), milliseconds; lower is better)

| language | fib | loop_sum | string_build | vec_memory |
| --- | ---: | ---: | ---: | ---: |
| lsaca | 30.6 (29.1) | 11.4 (10.9) | 212.9 (210.4) | 86.1 (83.5) |
| C (gcc -O2) | 30.6 (29.1) | 10.9 (10.8) | 14.8 (14.2) | 18.4 (17.1) |
| Rust (rustc -O) | 15.7 (15.1) | 12.0 (10.8) | 76.3 (69.5) | 24.9 (22.6) |
| Python 3 | 1082.3 (1035.2) | 987.1 (979.8) | 431.1 (422.6) | 535.7 (522.9) |
| Node.js | 241.4 (236.9) | 165.3 (141.4) | 231.5 (223.9) | 241.2 (236.5) |

## Peak memory (MiB, peak working set; lower is better)

| language | fib | loop_sum | string_build | vec_memory |
| --- | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 35.1 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.3 | 18.9 |
| Python 3 | 11.0 | 11.0 | 74.1 | 142.2 |
| Node.js | 43.0 | 45.7 | 89.3 | 121.4 |

## Workloads

- fib: recursive fibonacci(30), called 10 times, checksummed; expected stdout checksum=8320400
- loop_sum: integer sum 1..=8,000,000; expected stdout checksum=32000004000000
- string_build: append 1,000,000 numeric tokens (i % 1000) to a string; checksum is length,sum-of-token-values; expected stdout checksum=3890000,499500000
- vec_memory: build a vector of 1,000,000 Point{x=i64, y=i64}, then checksum; expected stdout checksum=1499998500000

