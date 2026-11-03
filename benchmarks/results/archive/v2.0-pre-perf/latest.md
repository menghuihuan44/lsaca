# lsaca cross-language benchmarks

Generated: 2026-11-02T22:41:49+08:00  
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

## Compile time (seconds, median of 2 runs; lower is better)

| program | lsaca cold build (default back end) | lsaca cached build | lsaca emit-c + gcc | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| string_build | 2.610 | 0.058 | n/a | 0.326 | 0.411 | n/a | n/a |
| vec_memory | 2.477 | 0.069 | n/a | 0.562 | 0.400 | n/a | n/a |
| binary_trees | 2.484 | 0.052 | n/a | 0.447 | 0.647 | n/a | n/a |
| binary_trees_refs | 2.104 | 0.046 | n/a | 0.355 | 0.415 | n/a | n/a |
| string_split | 2.492 | 0.058 | n/a | 0.343 | 0.495 | n/a | n/a |
| iterator_pipeline | 2.314 | 0.059 | n/a | 0.278 | 0.307 | n/a | n/a |
| hashmap_wordcount | 2.211 | 0.048 | n/a | 0.454 | 0.527 | n/a | n/a |
| generic_stack | 2.157 | 0.048 | n/a | 0.304 | 0.411 | n/a | n/a |
| fn_pipeline | 2.707 | 0.052 | n/a | 0.271 | 0.324 | n/a | n/a |
| string_unicode | 2.208 | 0.054 | n/a | 0.558 | 0.578 | n/a | n/a |
| hashmap_i64 | 2.500 | 0.043 | n/a | 0.502 | 0.669 | n/a | n/a |
| hashmap_string | 2.567 | 0.057 | n/a | 0.346 | 0.863 | n/a | n/a |

> lsaca cold build: the default `lsaca build -r`, repeated --compile-repeats times (default 3), each repeat starting from a fresh LSACA_CACHE_DIR. Since 1.2 the default build back end is the compiler's built-in direct C compiler back end: it emits the generated C and compiles it together with the cached runtime translation units using gcc. The cached build repeats the command against the same cache directory (the runtime objects and the content-addressed project are reused, so it mostly just re-links). The compiler still supports the older CMake + Ninja path via `lsaca build -r --cmake`; the harness does not measure it by default. Cold and cached compile times are medians of --compile-repeats runs (default 3); latest.json stores every run plus the minimum and maximum.

## Runtime (median (min), milliseconds; lower is better)

| language | string_build | vec_memory | binary_trees | binary_trees_refs | string_split | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 70.8 (50.9) | 32.6 (28.3) | 545.3 (448.9) | 37.1 (34.2) | 35.3 (33.8) | 35.7 (35.2) | 60.9 (57.4) | 23.6 (21.6) | 16.5 (15.6) | 375.0 (270.1) | 262.8 (252.1) | 377.0 (338.4) |
| C (gcc -O2) | 14.4 (13.5) | 24.0 (22.4) | 29.1 (26.8) | 20.8 (20.0) | 14.3 (13.9) | 14.4 (13.9) | 31.5 (29.1) | 17.1 (15.5) | 11.6 (10.6) | 18.8 (16.8) | 203.7 (193.3) | 253.3 (232.5) |
| Rust (rustc -O) | 73.0 (68.7) | 20.5 (19.5) | 36.9 (24.5) | 23.8 (21.6) | 16.0 (14.8) | 14.2 (12.0) | 52.1 (49.5) | 28.8 (20.0) | 10.4 (9.9) | 18.0 (16.8) | 228.5 (206.0) | 293.6 (248.1) |
| Python 3 | 409.7 (408.1) | 661.8 (569.6) | 117.0 (112.0) | 110.5 (105.8) | 84.3 (81.5) | 273.2 (226.6) | 132.8 (125.3) | 511.6 (410.8) | 41.1 (38.8) | 262.9 (255.0) | 2491.6 (2248.1) | 1122.7 (1056.8) |
| Node.js | 263.8 (236.2) | 327.1 (312.2) | 162.2 (154.9) | 158.8 (148.2) | 214.1 (178.3) | 181.6 (168.2) | 158.0 (155.0) | 169.7 (164.0) | 131.0 (129.0) | 245.1 (244.3) | 1052.9 (898.5) | 426.6 (401.3) |

## Peak memory (MiB, peak working set; lower is better)

| language | string_build | vec_memory | binary_trees | binary_trees_refs | string_split | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 17.3 | 20.1 | 53.5 | 17.0 | 10.7 | 12.5 | 5.4 | 13.8 | 5.1 | 6.8 | 56.7 | 33.6 |
| C (gcc -O2) | 7.3 | 18.8 | 7.6 | 7.6 | 4.0 | 11.1 | 4.2 | 12.6 | 4.0 | 4.3 | 52.4 | 19.7 |
| Rust (rustc -O) | 7.4 | 18.9 | 7.7 | 7.7 | 4.4 | 11.2 | 4.4 | 12.7 | 4.1 | 4.7 | 55.9 | 19.8 |
| Python 3 | 74.5 | 142.1 | 21.0 | 21.0 | 17.4 | 49.2 | 11.4 | 54.2 | 11.1 | 27.0 | 113.4 | 38.0 |
| Node.js | 88.4 | 121.4 | 56.3 | 56.8 | 50.7 | 57.9 | 46.6 | 72.5 | 39.9 | 68.1 | 102.6 | 90.9 |

## Runtime by data scale (median (min), milliseconds; lower is better)

| program | scale | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| string_build | small | 20.5 (19.4) | 12.6 (11.1) | 12.1 (10.6) | 57.4 (49.5) | 175.9 (168.0) |
| string_build | medium | 70.8 (50.9) | 14.4 (13.5) | 73.0 (68.7) | 409.7 (408.1) | 263.8 (236.2) |
| string_build | large | 145.5 (136.4) | 32.0 (29.8) | 240.0 (239.4) | 1966.1 (1785.9) | 608.3 (514.1) |
| binary_trees | small | 32.1 (26.5) | 17.5 (16.2) | 18.1 (15.6) | 45.3 (42.2) | 152.3 (150.1) |
| binary_trees | medium | 545.3 (448.9) | 29.1 (26.8) | 36.9 (24.5) | 117.0 (112.0) | 162.2 (154.9) |
| binary_trees | large | 2147.6 (1984.1) | 72.9 (62.2) | 60.5 (59.8) | 354.5 (351.0) | 193.2 (192.6) |
| binary_trees_refs | small | 17.7 (17.0) | 13.0 (11.2) | 13.9 (12.0) | 43.3 (42.1) | 144.0 (138.3) |
| binary_trees_refs | medium | 37.1 (34.2) | 20.8 (20.0) | 23.8 (21.6) | 110.5 (105.8) | 158.8 (148.2) |
| binary_trees_refs | large | 160.1 (151.8) | 55.3 (54.2) | 55.7 (51.8) | 360.0 (339.7) | 197.9 (185.8) |
| string_split | small | 22.5 (21.1) | 15.0 (12.6) | 15.3 (13.5) | 52.4 (47.8) | 167.2 (166.2) |
| string_split | medium | 35.3 (33.8) | 14.3 (13.9) | 16.0 (14.8) | 84.3 (81.5) | 214.1 (178.3) |
| string_split | large | 213.6 (205.8) | 54.7 (53.0) | 62.1 (59.0) | 510.1 (481.3) | 380.6 (368.8) |
| hashmap_wordcount | small | 22.1 (19.2) | 13.0 (11.3) | 16.7 (15.1) | 47.7 (44.6) | 175.5 (171.8) |
| hashmap_wordcount | medium | 60.9 (57.4) | 31.5 (29.1) | 52.1 (49.5) | 132.8 (125.3) | 158.0 (155.0) |
| hashmap_wordcount | large | 276.6 (262.0) | 97.7 (96.6) | 212.8 (206.8) | 472.3 (472.2) | 273.6 (270.4) |

> The medium scale reuses the primary measurement above (default arguments); the small and large scales pass explicit arguments, listed under Data scales below.

## Artifact sizes (bytes)

| program | lsaca executable (bytes) | lsaca generated C (bytes) | C executable (bytes) | Rust executable (bytes) |
| --- | ---: | ---: | ---: | ---: |
| string_build | 331,995 | 11,749 | 140,382 | 142,336 |
| vec_memory | 330,766 | 8,595 | 139,058 | 132,096 |
| binary_trees | 336,121 | 18,087 | 143,819 | 142,336 |
| binary_trees_refs | 336,633 | 17,606 | 143,839 | 142,336 |
| string_split | 332,130 | 17,395 | 142,849 | 142,848 |
| iterator_pipeline | 382,296 | 186,208 | 139,253 | 132,096 |
| hashmap_wordcount | 335,734 | 22,859 | 143,556 | 145,920 |
| generic_stack | 335,239 | 21,134 | 142,725 | 133,632 |
| fn_pipeline | 378,752 | 169,731 | 139,238 | 132,096 |
| string_unicode | 337,047 | 37,612 | 142,035 | 140,800 |
| hashmap_i64 | 334,751 | 22,200 | 140,381 | 136,192 |
| hashmap_string | 337,162 | 28,275 | 143,360 | 138,240 |

## Independent oracles

> scripts/oracles.py reimplements every executed workload and recomputes its checksum line without running lsaca: 12 program checksums and 15 data-scale checksums agree with the expected output (mismatches: 0).

## Workloads

- string_build: append 1,000,000 numeric tokens (i % 1000) to a string; checksum is length,sum-of-token-values; expected stdout checksum=3890000,499500000
- vec_memory: build a vector of 1,000,000 Point{x=i64, y=i64}, then checksum; expected stdout checksum=1499998500000
- binary_trees: build a perfect binary tree of depth 16 (131,071 nodes) and sum all payloads; expected stdout checksum=2147450880
- binary_trees_refs: the same perfect binary tree of depth 16 (131,071 nodes) as binary_trees, but summed through borrowed references (&Option<Box<Node>>) instead of by value, so the traversal does not copy the tree; checksum matches binary_trees; expected stdout checksum=2147450880
- string_split: build a 194,500-byte comma-separated numeric string, split it on commas, parse every token and sum; checksum is len,count,sum; expected stdout checksum=194500,50000,24975000
- iterator_pipeline: run 1,000,000 i64 values through the lazy chain filter(even).map(x * x).fold(0, add) and print the folded sum; measures iterator adapter overhead against a hand-written loop; expected stdout checksum=166666166667000000
- hashmap_wordcount: count 200,000 deterministic words drawn from a 4,096-word alphabet in a hash map, then fold the squared counts; checksum is the sum of squared counts, the number of distinct keys and the highest frequency (order-independent); expected stdout checksum=9766208,4096,49
- generic_stack: the same generic stack instantiated for i64 and for String: push/pop 1,000,000 integers and 1,000 strings; checksum is integer sum and total string byte length; expected stdout checksum=499999500000,5900
- fn_pipeline: rule records that carry a label String and a fn(i64) -> i64 value in one struct, applied 1,000 times each plus a clone of the first rule; checksum is the result sum and total label byte length; expected stdout checksum=53637,22
- string_unicode: build a 120,000-scalar mixed ASCII/Unicode string one char at a time, fold every scalar from iteration and from 1,000 random char_at accesses, apply full-Unicode to_uppercase, slice on character boundaries, push/pop and extend; checksum is char count, byte length and the folded scalar checksum; expected stdout checksum=3357,6921,368454760
- hashmap_i64: 1,000,000 integer-key inserts, 1,000,000 present lookups, 1,000,000 absent lookups and 100,000 deletions in a hash map; checksum is the folded value sum, the final length and the number of successful lookups; expected stdout checksum=821060469,900000,1000000
- hashmap_string: 200,000 distinct generated String keys inserted into a hash map, then 200,000 present lookups, 200,000 absent lookups (folding the bytes of each absent key) and 20,000 deletions; checksum is the folded value sum, the final length and the number of successful lookups; expected stdout checksum=794025725,180000,200000

## Data scales

- string_build: small: args 10000 -> checksum=38900,4995000; medium: args (default) -> checksum=3890000,499500000; large: args 4000000 -> checksum=15560000,1998000000
- binary_trees: small: args 10 -> checksum=523776; medium: args (default) -> checksum=2147450880; large: args 18 -> checksum=34359607296
- binary_trees_refs: small: args 10 -> checksum=523776; medium: args (default) -> checksum=2147450880; large: args 18 -> checksum=34359607296
- string_split: small: args 1000 -> checksum=3890,1000,499500; medium: args (default) -> checksum=194500,50000,24975000; large: args 500000 -> checksum=1945000,500000,249750000
- hashmap_wordcount: small: args 10000 -> checksum=25424,4096,3; medium: args (default) -> checksum=9766208,4096,49; large: args 1000000 -> checksum=244141120,4096,245

