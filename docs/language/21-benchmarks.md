# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
intuition: the same eighteen workloads are implemented in lsaca, C, Rust, Python and
Node.js, built with each language's normal toolchain, run on the same machine, and checked
against a shared expected output. A nineteenth, larger source is measured for compile time
only.

The numbers below were measured on **2026-10-31** with the repository's harness,
`scripts/bench.py`, on the reference machine described in
[Environment](#environment), using lsaca 1.6.0. The raw data (including every individual
run, not just the medians) is in
[benchmarks/results/latest.json](../../benchmarks/results/latest.json);
the harness also writes a machine-generated subset of these tables to
[benchmarks/results/latest.md](../../benchmarks/results/latest.md). Instructions for
reproducing the run are at the end of this page.

The short version for lsaca 1.6.0:

- **Scalar loops and recursion are in the same ballpark as C.** `loop_sum` is 9.6 ms in
  lsaca versus 8.9 ms in C, `fib` 25.3 versus 24.5 ms, `sieve` 32.6 versus 28.0 ms,
  `mandelbrot` 24.9 versus 24.3 ms and `fn_pipeline` 7.6 versus 7.2 ms. These are
  sub-35 ms workloads measured on an otherwise idle machine; read the rows as "same
  range as C" and compare ratios inside a single run rather than absolute times across
  pages.
- **`vec_memory` is within 1.1x of C**: 12.7 ms and 18.8 MiB peak versus C's 12.3 ms /
  18.8 MiB and Rust's 13.8 ms / 18.9 MiB. In 1.1.0 the same workload took 86.1 ms and
  35.1 MiB, so the runtime and code-generation work since then still holds.
- **`string_build` is 2.6x slower than C but 2.0x faster than Rust here** (28.6 ms
  versus C's 11.0 ms and Rust's 56.4 ms). In 1.1.0 it took 212.9 ms, so the 1.2 runtime
  work still shows a 7.4x improvement.
- **Ownership keeps the cost of copying explicit.** `binary_trees_refs` sums the tree
  through `&Option<Box<Node>>` and copies nothing: 26.8 ms and 15.8 MiB, which is 1.67x
  C's runtime and 2.06x its peak memory. The value-taking `binary_trees` variant has to
  `.clone()` each child subtree; it pays 322.4 ms and 52.3 MiB - 12.0x slower and 3.3x
  more peak memory than the borrowed traversal. That gap is the measured cost of value
  semantics used naively, and it narrowed in 1.6 (see "What 1.6 changed" below).
- **Generics, iterators and HashMap stay close to hand-written code.** A generic
  `Stack<T>` monomorphizes to code at parity with a hand-written C stack (12.8 versus
  12.8 ms); a struct carrying a function value matches C function pointers (7.6
  versus 7.2 ms); a lazy `filter/map/fold` chain costs 1.99x a hand-written C loop (25.4
  versus 12.8 ms) but allocates nothing extra (11.2 MiB peak, same as the loop). The
  1.6 Swiss-Table `HashMap` is 2.00x a hand-written open-addressing C map on the word
  count (46.8 versus 23.4 ms); the two new dedicated map workloads are 1.59x C for
  `i64` keys (211.3 versus 133.0 ms) and 1.46x C for `String` keys (241.7 versus 165.8
  ms).
- **A cold build takes 0.91-1.22 s** for the small programs; the default direct C
  compiler back end emits C and drives gcc itself. A cached build is 14-21 ms. The
  4,511-line `compile_large` source takes 1.83 s cold and 244 ms cached, and its
  generated C is split across `main.c` plus three `lsaca_unit_N.c` files; `main.c`
  alone records 395,372 bytes (about 0.40 MB) and the whole program's generated C is
  about 1.17 MB.
- **Interpreters are 4.46-102.66x slower than C** on these workloads; their numbers
  also carry interpreter/VM start-up on every run.

## Workloads

Eighteen small programs, each of which prints a single `checksum=...` line, plus a large
generated source used only to measure how build time scales with program size. The
workloads were chosen to be simple enough that every language can express the same
algorithm without exotic workarounds, while still stressing different parts of a runtime.

| workload | what it stresses | exact workload | expected stdout |
| --- | --- | --- | --- |
| `fib` | non-tail recursion and calls | `fib(30)` computed 10 times and summed; `n` is a constant | `checksum=8320400` |
| `loop_sum` | tight scalar integer loop | sum of 1..=8,000,000 | `checksum=32000004000000` |
| `string_build` | building a string from many small pieces | append 1,000,000 decimal tokens `i % 1000`, each followed by a comma; checksum is the final byte length and the sum of the token values | `checksum=3890000,499500000` |
| `vec_memory` | allocation volume and peak memory | build a vector of 1,000,000 `Point { x: i64, y: i64 }` values with `x = i`, `y = 2*i`; checksum over `x + y` | `checksum=1499998500000` |
| `sort_i64` | in-language sorting of random data | 200,000 pseudo-random `i64` values (LCG, seed 12345), sorted with an iterative quicksort written in the language; checksum is first, last and sum | `checksum=21749,2147481261,214813518297651` |
| `matrix_multiply` | integer multiply-add loops | multiply two 160x160 `i64` matrices (flat vectors, naive triple loop); checksum is the sum of the result entries | `checksum=1018924712000` |
| `binary_trees` | many small allocations, recursion | build a perfect binary tree of depth 16 (131,071 nodes) and sum every payload through a by-value function that `.clone()`s each child subtree; the deep copies are part of the measured cost | `checksum=2147450880` |
| `binary_trees_refs` | borrowed traversal | the same tree as `binary_trees`, but summed through borrowed references (`&Option<Box<Node>>`) instead of by value, so the traversal does not copy the tree | `checksum=2147450880` |
| `string_split` | splitting and parsing text | build a 194,500-byte comma-separated numeric string, split it, parse every token and sum; checksum is length, token count and sum | `checksum=194500,50000,24975000` |
| `sieve` | array-heavy loop | sieve of Eratosthenes below 2,000,000; count the primes | `checksum=148933` |
| `mandelbrot` | floating-point loops | 600x400 escape-time Mandelbrot, at most 100 iterations per pixel; checksum is the sum of the iteration counts | `checksum=6282265` |
| `iterator_pipeline` | lazy iterator abstraction versus a loop | run 1,000,000 `i64` values through `iter().filter(even).map(x*x).fold(0, add)`; the checksum is the folded sum | `checksum=166666166667000000` |
| `hashmap_wordcount` | keyed collection with owned keys | count 200,000 deterministic words drawn from a 4,096-word alphabet in a `HashMap<String, i64>`; checksum is the sum of squared counts, the number of distinct keys and the highest frequency | `checksum=9766208,4096,49` |
| `generic_stack` | generic data structure, two instantiations | one generic `Stack<T>` instantiated for `i64` (push/pop 1,000,000 values) and for `String` (1,000 values); checksum is the integer sum and the total string byte length | `checksum=499999500000,5900` |
| `fn_pipeline` | function values stored in records | three rule records `{ label: String, apply: fn(i64) -> i64 }`, each applied 1,000 times, plus a clone of the first rule applied 500 times; checksum is the result sum and total label byte length | `checksum=53637,22` |
| `string_unicode` | character-level string building and Unicode data | build a 120,000-scalar mixed ASCII/Unicode string one character at a time, fold every scalar from iteration and from 1,000 random `char_at` accesses, apply full-Unicode `to_uppercase`, slice on character boundaries, push/pop and extend; checksum is character count, byte length and the folded scalar checksum | `checksum=3357,6921,368454760` |
| `hashmap_i64` | integer-key map operations | 1,000,000 integer-key inserts, 1,000,000 present lookups, 1,000,000 absent lookups and 100,000 deletions; checksum is the folded value sum, the final length and the number of successful lookups | `checksum=821060469,900000,1000000` |
| `hashmap_string` | string-key map: hashing and owned keys | 200,000 distinct generated `String` keys inserted, then 200,000 present lookups, 200,000 absent lookups (folding the bytes of each absent key) and 20,000 deletions; checksum is the folded value sum, the final length and the number of successful lookups | `checksum=794025725,180000,200000` |
| `compile_large` | compile-time scaling with source size | generated 4,511-line lsaca source (450 functions); **compile-only**, never executed by the harness | `checksum=14655037` (not checked) |

`string_unicode`, `hashmap_i64` and `hashmap_string` are new in 1.6.0;
`iterator_pipeline`, `hashmap_wordcount`, `generic_stack` and `fn_pipeline` were added
in 1.5.0; the other eleven runtime workloads carry over from earlier releases, so the
1.5 and 1.4 session figures quoted in the analysis below remain directly comparable.

The sizes are deliberate. `loop_sum` uses 8,000,000 iterations so that Python takes about
one second: long enough to measure above start-up noise, short enough that seven runs per
workload keep the whole suite under a few minutes. The other workloads are sized so the
fastest native implementation takes tens of milliseconds, which is where process start-up
is still small compared with the work.

Sources: [benchmarks/programs/](../../benchmarks/programs/), one file per workload and
language (`.ls`, `.c`, `.rs`, `.py`, `.js`), plus the generator for
`compile_large` (`gen_compile_large.py`).

## What is measured

### Compile time

Wall-clock time of one build command, median of 3 repetitions (1 in `--quick` mode):

- **lsaca, cold build**: `lsaca build -r <source> -o <outdir>` with a **fresh**
  `LSACA_CACHE_DIR` for each measurement. This is the full user-facing path: emit C,
  compile the generated program units (`main.c`, or `main.c` plus
  `lsaca_unit_1.c` … `lsaca_unit_N.c` when a very large program is split), compile the
  six runtime translation units, and link,
  all with the compiler's default direct C compiler back end (gcc, `-O2 -DNDEBUG`). The
  runtime objects are compiled once into the cache root and reused by later builds; a
  fresh cache directory forces the full work.
- **lsaca, cached build**: the same command immediately afterwards, with the cache
  directory retained. The compiler keys its build on a content hash, finds the previously
  built executable and returns it without recompiling program or runtime.
- **C**: `gcc -O2 <source> -o <exe>`.
- **Rust**: `rustc -O <source> -o <exe>`.
- **Python / Node.js**: not applicable - these languages have no ahead-of-time compile
  step in this suite. Their start-up cost is part of the runtime numbers instead.

Since 1.2.0 the default build path does not use CMake at all: the compiler invokes gcc
directly. The older CMake + Ninja pipeline is still available as
`lsaca build -r --cmake`, but the harness does not measure it. Not included in any
compile figure: building the lsaca compiler itself (`cargo build --release -p lsaca-cli`),
or the one-time toolchain setup on a brand-new machine. The harness sets
`TEMP`/`TMP`/`TMPDIR` to a directory inside `target/bench-tmp` and puts the usual
toolchain directories on `PATH` (`C:\msys64\ucrt64\bin`, `C:\Program Files\CMake\bin`,
`~/.cargo/bin` on Windows) so the numbers do not depend on the caller's shell
configuration.

### Runtime

Each program runs as a **fresh child process**. The harness does 1 unmeasured warm-up run
and then 7 measured runs (3 in `--quick` mode), and reports the **median** and the
**minimum** of the measured runs. Every measured run's stdout is validated against the
expected line; a mismatch or a non-zero exit aborts the whole run.

- Process start-up (loader, runtime initialisation, interpreter/VM startup) is included.
  It is small for native programs and not small for Python/Node.js, which is part of what
  the table shows.
- No CPU pinning, no warm-up beyond the one warm-up run; the machine is a normal laptop
  with power management and background activity (see the caveats).

### Peak memory

Peak memory is measured in a **separate run** of the same executable, by sampling the
peak working set of the child process:

- Windows: polling `GetProcessMemoryInfo` / `PeakWorkingSetSize` (psapi) while the child
  runs, with a PowerShell `PeakWorkingSet64` fallback.
- Linux: `/usr/bin/time -v` (maximum resident set size) with a `/proc/<pid>/status`
  `VmHWM` fallback; macOS: `/usr/bin/time -l` with a `/proc` fallback where available.

This is **peak resident memory**, not bytes allocated, so it includes the language runtime
and the interpreter/VM baseline. It is a sampled peak, not an exact byte count; a spike
shorter than the sampling interval can be missed.

### Validation

All 91 (workload, language) combinations were built (90 of them also run), and every
measured run matched its expected checksum line. The machine-readable result says
`validation: "all measured runs matched the expected checksum"`. `compile_large` is
built but not executed, so it has no runtime or memory rows. Because the checksums cover
the whole workload, an implementation that silently skipped work would fail the run
instead of appearing "fast".

This run recorded no warnings: every sample in the tables is an unmodified measurement
from the same session.

## Environment

Everything in the table was collected by the harness at run time and is stored in
`benchmarks/results/latest.json`, together with the individual run times.

| field | value |
| --- | --- |
| Measured at | 2026-10-31 23:24:44 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 1.6.0, repository HEAD `59d56c7`, compiler sha256 prefix `b1489a3dec37e163`, compiler mtime 2026-10-31 23:03:09 +08:00 |
| lsaca build path | `lsaca build -r`, default direct C compiler back end (gcc 15.2.0, `-O2 -DNDEBUG`); the old CMake + Ninja path is available as `--cmake` |
| C compiler | gcc 15.2.0 (MSYS2 UCRT64, Rev14, Built by MSYS2 project) |
| Rust compiler | rustc 1.99.0 (`b940084d7 2026-09-28`) |
| Python | 3.14.4 |
| Node.js | v24.14.1 |
| Harness | `scripts/bench.py`, 19 programs (18 executed), 1 warm-up + 7 runs, 3 compile repeats, peak-memory sampling on |

The lsaca executable used for the run is a copy of `target/release/lsaca.exe`
(`target/bench-tools/lsaca.exe`); the harness records its SHA-256 prefix so a result can
be tied to a specific compiler binary. The compiler does not need to be rebuilt for a
benchmark run, but the harness does copy it at the start of every run.

## Results

### Compile time

Seconds, median of 3 runs. Lower is better. "cold" and "cached" are two different
questions - a first build versus a rebuild with the compiler's build cache already warm.

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

The "lsaca emit-c + gcc" column is only filled when the harness runs in fallback mode
(see the fairness notes); in this official run the default build back end was used and
that column is not measured.

Observations:

- A cold lsaca build is **0.91-1.22 s** for the eighteen runtime workloads (the fastest
  is `string_split` at 0.907 s, the slowest `sort_i64` at 1.219 s). The 1.5 session
  measured 0.74-1.05 s, the 1.4 suite 1.03-1.32 s and the 1.3 suite 1.15-1.72 s. This
  run was taken on an otherwise idle machine, but absolute compile times still drift with
  machine state and compiler layout changes, so compare within a session. Most of the
  cold cost is gcc: compiling the generated
  `main.c` together with the six runtime translation units and linking them. The front
  end itself is a small part of the measurement - the C emission step is not separately
  recorded but is a few tens of milliseconds.
- `compile_large` shows the scaling: 4,511 lines / 450 functions produce 395,372 bytes
  (about 0.40 MB) of program-specific C in `main.c`, plus the function bodies in three
  generated units (`lsaca_unit_1.c` ... `lsaca_unit_3.c`), and 1.83 s of cold build
  time. Its emitted user code exceeds the 256 KiB split threshold, so it is the only
  workload compiled from several generated units. If `main.c` alone is counted - as the
  harness does - the generated-C figure is 395,372 bytes; including the three units it is
  about 1.17 MB. In the 1.5 session the same source was emitted as one `main.c` of
  1,046,885 bytes and took 3.14 s cold; splitting the C across units made the build
  roughly 1.7x faster even though the runtime grew a Unicode translation unit.
- A cached lsaca build is **14-21 ms** for the small programs (244 ms for
  `compile_large`). That is the content-addressed cache hit: the compiler hashes the
  generated program and returns the existing executable rather than invoking gcc. In
  practice an edit to the `.ls` source invalidates the cache and pays the full cold cost
  again; there is no incremental compile yet.
- The C column (0.21-0.29 s) is a single-file compile with no project setup; it is the
  right baseline for "front end plus codegen", not for the whole emit-compile-link
  pipeline. In this session `rustc -O` took 0.21-0.56 s, comparable to gcc; earlier
  sessions on the same machine measured 0.93-1.65 s for the same rustc invocations (see
  the caveats), which is a reminder that absolute compile times here are sensitive to
  the machine's state, not to these benchmark sources.

### Runtime

Milliseconds, **median (minimum)** of 7 measured runs after 1 warm-up. Lower is better.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 25.3 (25.0) | 9.6 (9.1) | 28.6 (28.0) | 12.7 (12.3) | 37.0 (32.1) | 12.4 (11.9) | 322.4 (315.9) | 26.8 (24.6) | 16.0 (15.2) | 32.6 (31.1) | 24.9 (23.0) | 25.4 (24.4) | 46.8 (41.8) | 12.8 (12.0) | 7.6 (7.4) | 27.9 (25.9) | 211.3 (202.8) | 241.7 (233.8) |
| C (gcc -O2) | 24.5 (23.1) | 8.9 (8.7) | 11.0 (10.5) | 12.3 (12.1) | 33.8 (32.2) | 12.6 (12.0) | 16.0 (15.1) | 16.1 (15.4) | 10.5 (9.9) | 28.0 (26.5) | 24.3 (23.4) | 12.8 (10.8) | 23.4 (21.8) | 12.8 (12.2) | 7.2 (6.9) | 10.0 (9.7) | 133.0 (131.4) | 165.8 (162.3) |
| Rust (rustc -O) | 12.9 (10.7) | 9.3 (8.3) | 56.4 (53.4) | 13.8 (13.0) | 31.7 (29.9) | 12.1 (11.6) | 17.6 (16.6) | 17.3 (16.6) | 10.7 (10.3) | 26.5 (25.8) | 24.1 (23.7) | 15.3 (13.6) | 43.2 (40.0) | 14.5 (13.5) | 8.1 (7.3) | 11.7 (11.2) | 172.1 (166.6) | 246.8 (241.3) |
| Python 3 | 983.7 (804.8) | 912.2 (817.1) | 347.1 (343.8) | 422.5 (415.0) | 632.3 (609.4) | 1149.7 (1128.2) | 86.1 (82.3) | 85.6 (79.2) | 57.7 (55.4) | 467.1 (453.3) | 2215.3 (2181.2) | 218.5 (205.0) | 104.4 (97.6) | 301.3 (291.0) | 32.5 (31.2) | 222.1 (209.1) | 1722.3 (1688.5) | 995.9 (912.1) |
| Node.js | 213.4 (210.0) | 109.3 (106.7) | 171.8 (162.7) | 227.0 (201.4) | 162.2 (161.0) | 116.5 (114.5) | 113.1 (107.4) | 113.2 (109.9) | 113.0 (108.8) | 142.0 (124.6) | 127.0 (122.9) | 136.7 (133.5) | 125.6 (121.3) | 129.2 (124.1) | 104.4 (102.2) | 158.3 (148.6) | 608.5 (596.3) | 370.4 (343.5) |

The same numbers as a ratio to the C time (median / C median). Below 1.0 means faster
than C:

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 1.03x | 1.08x | 2.60x | 1.03x | 1.09x | 0.99x | 20.15x | 1.67x | 1.52x | 1.16x | 1.03x | 1.99x | 2.00x | 0.99x | 1.06x | 2.78x | 1.59x | 1.46x |
| Rust | 0.53x | 1.05x | 5.13x | 1.12x | 0.94x | 0.96x | 1.10x | 1.08x | 1.02x | 0.95x | 0.99x | 1.20x | 1.84x | 1.13x | 1.13x | 1.17x | 1.29x | 1.49x |
| Python 3 | 40.22x | 102.66x | 31.53x | 34.28x | 18.69x | 91.40x | 5.38x | 5.33x | 5.49x | 16.66x | 91.34x | 17.12x | 4.46x | 23.46x | 4.52x | 22.18x | 12.95x | 6.01x |
| Node.js | 8.72x | 12.30x | 15.60x | 18.42x | 4.79x | 9.26x | 7.07x | 7.05x | 10.76x | 5.06x | 5.24x | 10.71x | 5.36x | 10.06x | 14.52x | 15.80x | 4.58x | 2.23x |

### Peak memory

MiB of peak working set, separate run. Lower is better. Remember that this is total
process footprint: it includes the runtime, the interpreter/VM and its heap.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline | string_unicode | hashmap_i64 | hashmap_string |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.1 | 4.1 | 10.5 | 18.8 | 6.6 | 4.4 | 52.3 | 15.8 | 9.3 | 18.9 | 4.1 | 11.2 | 4.2 | 12.6 | 4.1 | 5.4 | 55.5 | 32.3 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.4 | 7.7 | 7.7 | 4.1 | 18.9 | 4.0 | 11.2 | 4.3 | 12.6 | 4.0 | 4.3 | 52.4 | 19.7 |
| Rust (rustc -O) | 4.1 | 4.2 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 | 11.3 | 4.4 | 12.7 | 4.2 | 4.7 | 55.9 | 19.8 |
| Python 3 | 11.0 | 11.0 | 74.1 | 142.8 | 21.4 | 13.6 | 21.1 | 21.0 | 16.8 | 26.2 | 11.0 | 49.3 | 11.4 | 54.2 | 11.1 | 26.0 | 113.4 | 38.0 |
| Node.js | 43.0 | 45.8 | 89.4 | 121.4 | 50.4 | 47.6 | 56.4 | 56.4 | 50.8 | 60.2 | 44.7 | 58.1 | 46.4 | 72.3 | 40.3 | 68.2 | 102.4 | 90.7 |

For workloads with no large data structures (`fib`, `loop_sum`, `mandelbrot`,
`fn_pipeline`) every native language sits at the 4 MiB floor of the process image, and
`generic_stack` stays at 12.6-12.7 MiB for the three native languages. Where the data is
large: `vec_memory` allocates 16 MB of `Point` values and both lsaca and C peak at
18.8 MiB; `sieve` keeps a 2,000,000-element `i64` flag vector, 16 MB, and peaks at
18.9 MiB; `string_build` allocates a ~3.9 MB string and adds a temporary per token,
hence 10.5 MiB versus 7.3 MiB for C. `string_split` peaks at 9.3 MiB versus 4.1 MiB
for C because the split tokens and the parsed values are heap `String`s on the lsaca
side; `hashmap_string` peaks at 32.3 MiB versus 19.7 MiB for C because every key is an
owned `String`. `binary_trees` is the outlier at 52.3 MiB for 131,071 small nodes,
because the by-value traversal clones whole subtrees; the borrowed `binary_trees_refs`
traversal copies nothing and peaks at 15.8 MiB, 2.06x C's 7.7 MiB instead of 6.8x.

## Analysis

### Scalar integer loops: same range as C

`loop_sum` is the cleanest comparison: 8,000,000 iterations of `sum += i` and nothing
else. lsaca takes 9.6 ms and C 8.9 ms (1.08x) while Rust measured 9.3 ms on the same
program this session. In release mode the generated C lowers integer additions to plain
C arithmetic (the wrapping semantics are expressed as explicit `uint64_t` casts, so the
compiler does not need to keep overflow checks), and GCC -O2 turns the loop into
comparable machine code to a hand-written C loop. This run landed near the 1.5 session on
this row (lsaca 9.6 ms versus 8.1 ms then, C 8.9 ms versus 9.2 ms), so compare the ratio:
1.08x here against 0.89x then.

`sieve` (32.6 vs 28.0 ms) and `mandelbrot` (24.9 vs 24.3 ms) tell the same story for an
array-heavy loop and for floating-point arithmetic: when the work does not need the
runtime's help, lsaca generates C that performs like C. This run lsaca came out behind
on both rows (1.16x and 1.03x C); in the 1.5 session the same pair measured 1.05x
and 0.96x, and at 30-40 ms the GCC-vs-LLVM code-generation noise and the machine state
dominate. This is the strongest evidence that the C back end is not intrinsically slow.

### Recursion-heavy calls: fib

The recorded session has lsaca at 25.3 ms, C at 24.5 ms and Rust at 12.9 ms for the same
recursive algorithm. Two things matter here, and only one of them is about lsaca:

1. **C and Rust differ by 1.9x on identical source-level algorithms.** That is a
   code-generation effect between GCC 15.2 and LLVM (via rustc 1.99.0) on a
   self-recursive function, not a language-semantics effect. Any single workload can
   show a compiler gap of this size; it should not be read as "Rust is faster than C".
2. **The lsaca / C difference moves between sessions.** This run lsaca came out at
   1.03x C; in the 1.5 session it was 0.95x, in the 1.4 session 0.82x, in the 1.3 session
   1.24x, and an earlier interleaved re-timing of a previous binary pair (9 runs each,
   alternating) measured 36.9 ms (lsaca) vs 39.0 ms (C) - roughly parity. Treat the fib
   row as "same ballpark", not as a stable speed ratio.

The constant argument n=30 does not explain the differences: probes with
`std::hint::black_box(30)` in Rust and a `volatile` n in C produced timings within a few
percent of the plain versions, so the compilers are not constant-folding the recursion. In
the generated lsaca program n is a runtime variable anyway.

### String building: runtime-bound, 2.6x C

`string_build` appends 1,000,000 small strings. lsaca needs 28.6 ms versus 11.0 ms for C
and 56.4 ms for Rust (Rust is slower than C here because it also allocates a temporary
`String` per token). In lsaca 1.1.0 the same program took 212.9 ms; the 1.2 runtime work
brought that down to 33.9 ms, and this run measures 28.6 ms - 7.4x faster than 1.1.0 and
2.0x faster than the Rust version. C measured 10.0 ms in the 1.5 session and 11.0 ms
here, so the ratio to C hardly moved (2.7x to 2.6x) even though lsaca itself was slightly
slower than the 27.0 ms of the 1.5 session: compare within a row, not across sessions.

The remaining gap to C is about what a `String` is in each implementation:

- The C program allocates one buffer of the exact final size up front and writes bytes
  into it. There are almost no function calls in the hot loop.
- At the lsaca level the program is `s.push_str(format!("{},", v))`. Each iteration
  builds a temporary formatted `String` and appends it through the runtime's string
  append function, which handles reference counting and geometric growth. The loop
  therefore performs one allocation plus one append per token; C's version performs none.
  Since 1.6 string literals are interned once per distinct literal and the
  shortest-round-trip float formatter is a Dragon4 implementation, but this workload
  formats integers, so those changes do not move it.
- The peak-memory row reflects the same design: the last growth step keeps the old and
  the new buffer alive at the same time (10.5 MiB total versus 7.3 MiB for C).

The measured gap is a statement about the current runtime implementation, not about the
language: an API that let the caller append into a pre-sized buffer, or that inlined the
growth path, could close most of it.

### Vectors of struct values: within 1.1x of C

`vec_memory` pushes 1,000,000 two-word structs into a `Vec`. In this session lsaca takes
12.7 ms and peaks at 18.8 MiB, versus 12.3 ms / 18.8 MiB in C and 13.8 ms / 18.9 MiB in
Rust - all three within 1.1x on time and equal on peak memory. In 1.1.0 it was 86.1 ms
and 35.1 MiB.

Two runtime and code-generation changes explain the difference: the hot `push` path is
now a fast path that writes the value into the reserved slot directly instead of going
through a temporary element and a generic insertion helper, and `for p in points`
consumes the vector since 1.4, so the loop does not deep-copy the whole buffer.

`Vec` is a **move-only value type** in lsaca: moving it (assignment, argument, return)
moves the buffer, and `.clone()` deep-copies it. Since 1.3 the language also has
references, so a helper function can take `&Vec<T>`/`&mut Vec<T>` and look at or mutate
the vector in place without copying it, and since 1.4 mutable heap references work too.
The vector in this workload is built and consumed in one function without such a copy, so
the benchmark shows that the common path is close to C. Shapes that do copy (see
`binary_trees` and the fairness notes) still pay for it, but the source now has to ask
for the copy.

### Sorting: 200,000 values, in-language algorithm

`sort_i64` sorts 200,000 pseudo-random values with an iterative quicksort written in the
language: explicit stack of index pairs, middle pivot, Hoare partition, manual swaps
through bounds-checked indexing. lsaca takes 37.0 ms versus 33.8 ms in C and 31.7 ms in
Rust, 1.09x C - in the same range, with Rust's `swap` version also close to C this
session. The C version uses the same algorithm and the same explicit stack.

No language uses a library sort in this workload (lsaca has no `Vec::sort`), so this is a
fair "how expensive are the language's primitives in a write-heavy loop" comparison, and
the answer is: in the same range as C. The swap-and-index loop executes bounds checks in
lsaca that C does not, but this workload is branch-heavy enough that its absolute time
moves more between sessions than that overhead; treat the row as "same range", not as a
measured constant. A library `qsort`/`sort_unstable` in C or Rust would be faster than
any of these numbers.

### Matrix multiplication: bounds checks in the inner loop

`matrix_multiply` multiplies two 160x160 `i64` matrices stored as flat vectors. lsaca
takes 12.4 ms versus 12.6 ms for C and 12.1 ms for Rust - parity with both this run (0.99x C; it was parity at 12.8 vs 12.7 ms in the 1.5 session). The inner loop indexes the two input
vectors and the output vector by computed positions
(`a[i*n+k] * b[k*n+j]`), so every access is bounds-checked in lsaca, while C and Rust
index without a check. Bounds checks of that shape are typically a few percent on a
workload this small; the session-to-session swing (and the fact that the whole program is
about 16 ms) means this row is dominated by code-generation and session state. The
arithmetic itself is plain C code, which is why all three implementations land in the
same range.

### Binary trees: value semantics versus the borrowed traversal

`binary_trees` builds a perfect tree of 131,071 nodes and sums every payload through a
value-taking function (`fn sum_tree(tree: Option<Box<Node>>) -> i64`). C takes 16.0 ms
and peaks at 7.7 MiB; lsaca takes 322.4 ms and peaks at 52.3 MiB. This is by far the
largest gap in the suite, and it is a language-model effect rather than a codegen effect:

- In C the tree is `malloc`ed nodes with raw pointers: building and traversing it never
  duplicates anything.
- In Rust the tree is `Box<Node>`: moving a `Box` is moving a pointer.
- In lsaca, move-only values are moved, so building the tree and passing it into
  functions does not duplicate it. But the traversal owns each node it matches and then
  needs a copy of its children: `Box::get()` returns an owned copy of the payload, and
  taking a child by value would move it out of a field, which is `E0507`, so the program
  clones the child subtrees explicitly with `.clone()`. Every level of the recursion
  therefore copies its subtree. The 20.2x runtime and the 6.8x peak-memory gaps are the
  price of that value-semantics shape, and the source shows the copies instead of hiding
  them in implicit copies.

The second workload uses exactly the same tree: `binary_trees_refs` sums it through
references instead - `fn sum_tree(tree: &Option<Box<Node>>) -> i64`, matching
`Some(boxed)` and recursing on `&current.left` and `&current.right`. That traversal does
not copy the tree at all. It takes 26.8 ms and peaks at 15.8 MiB, so **the borrowed traversal is 12.0x faster and uses 3.3x less peak memory than the value traversal on the same tree, same nodes and same algorithm**. It is also 1.67x C's runtime and 2.06x its peak memory - the same order as the rest of the suite rather than a factor of ten.

The borrowed version is not free of the runtime: it is still 2.06x C's peak memory,
because the tree is built and dropped out of reference-counted `Box` nodes. It does beat
the interpreters, though: Python takes 85.6 ms and Node.js 113.2 ms on the same borrowed
traversal, versus lsaca's 26.8 ms. Python's 86.1 ms on the by-value variant is still
much faster than lsaca's 322.4 ms, because Python objects are references and aliasing is
the default there.

### Iterators: a lazy pipeline against a loop

`iterator_pipeline` measures the iterator abstraction directly. It builds 1,000,000
`i64` values and then runs
`values.iter().filter(|x| *x % 2 == 0).map(|x| x * x).fold(0, |acc, x| acc + x)`.
The C version runs the same arithmetic as an eager `for` loop, Rust uses its iterator
chain, Python a generator expression and Node.js an eager loop.

lsaca takes 25.4 ms versus 12.8 ms for the C loop and 15.3 ms for Rust - the chain costs
1.99x a hand-written loop in the current implementation. That is the overhead of the
abstraction, not of allocation: the chain is lazy, so no intermediate vectors are
materialised, and its peak memory is 11.2 MiB, exactly the same as the eager C loop. In
the generated C each adapter is a generic struct with its own `next` function, so every
element passes through a call per adapter stage (filter, map) instead of one loop body;
Rust's optimizer inlines its equivalent chain completely, which is why Rust is not paying
that cost. The result is still 8.6x faster than Python (218.5 ms) and 5.4x faster than
Node.js (136.7 ms). A hand-written loop in lsaca would land close to C, as `loop_sum`
shows: the gap here is the price of the lazy chain, and it is the current implementation,
not a fundamental property of the language.

### HashMap: a Swiss-table builtin against a hand-written map

`hashmap_wordcount` counts 200,000 deterministic words drawn from a 4,096-word alphabet,
then folds the squared counts. The C version is a hand-written open-addressing map with
`malloc`ed `char *` keys; Rust uses `std::collections::HashMap` with `String` keys;
Python uses a dict and Node.js a `Map`.

lsaca takes 46.8 ms versus 23.4 ms for the C map (2.00x) and 43.2 ms for Rust's
`std::collections::HashMap` (1.08x slower than Rust). It beats Python's 104.4 ms by 2.2x
and Node's 125.6 ms by 2.7x, and its peak memory is 4.2 MiB - the same as the C map's
4.3 MiB. The gap to C is the price of ownership in the map interface: keys are move-only
`String`s, `get(key)` copies the key argument and clones the stored value, `insert`
moves the key and value in, and the checksum loop calls `keys()` (which clones every key
into a `Vec<String>`) and then `get` again for each key. Each of those operations is an
ORC retain/release or a small clone on a short string, where the C map passes a
`char *` and never touches a reference count. A C map is a fair baseline for the data
structure, and 2x is the measured cost of the managed-key API rather than of the hash
operation itself.

The two new 1.6 workloads measure the map without that word-count shape:
`hashmap_i64` performs 1,000,000 integer-key inserts, 1,000,000 present lookups,
1,000,000 absent lookups and 100,000 deletions; `hashmap_string` does the same with
200,000 distinct generated `String` keys. lsaca takes 211.3 ms for `hashmap_i64` versus
133.0 ms for a C open-addressing map and 172.1 ms for Rust (1.59x C), and 241.7 ms for
`hashmap_string` versus 165.8 ms in C and 246.8 ms in Rust (1.46x C). The string-key map
peaks at 32.3 MiB versus 19.7 MiB for C, because every key is an owned `String` on the
lsaca side; the integer-key map peaks at 55.5 MiB versus 52.4 MiB. The 1.6 map is a
Swiss-Table-style design (control bytes and 16-slot groups, 7/8 load factor, tombstones;
see [HashMap](20-hashmap.md)), but its layout change is about predictable growth and
worst-case behaviour, not a uniform speedup over the 1.5 open addressing: the
word-count ratio is close to the 1.5 session's, and the dedicated map workloads are what
the 1.6 numbers are for.

### Generic stack: monomorphization is free relative to C

`generic_stack` defines one `Stack<T>` type with `push`/`pop`/`len` methods and
instantiates it twice: for `i64` (1,000,000 pushes and pops) and for `String` (1,000
values). The C version writes the two typed stacks out by hand, since C has no generics.

lsaca takes 12.8 ms versus 12.8 ms for C (1.00x) and 14.5 ms for Rust, with all three
peaking at 12.6-12.7 MiB. Generics are monomorphized during type checking: each concrete
instantiation generates its own struct and functions in the emitted C, exactly like the
hand-written C versions, with no boxing and no dynamic dispatch. The row therefore shows
that the generic source form costs nothing at run time compared with the same code
written out for each type - the small gap is session noise.

### Function values in records

`fn_pipeline` stores a function value in a struct next to a non-copy `String` label:
`struct Rule { label: String, apply: fn(i64) -> i64 }`. It builds three rules, calls each
1,000 times, and calls a clone of the first rule 500 times. The C version uses a
`const char *` label and an `int64_t (*apply)(int64_t)` function pointer.

lsaca takes 7.6 ms versus 7.2 ms for C and 8.1 ms for Rust (1.06x C - all within
noise of each other), and peaks at 4.1 MiB versus C's 4.0 MiB. This is a workload that
was not expressible in 1.4 at all: storing a function value in a struct field produced
invalid C. Since 1.5 function values are ordinary aggregates, so the record can hold one
and the call goes through the stored handle; the cost is in the same range as a C
function pointer.

### Characters and Unicode strings

`string_unicode` is the 1.6 character-model workload: it builds a 120,000-scalar mixed
ASCII/Unicode string one character at a time, folds every scalar from iteration and from
1,000 random `char_at` accesses, applies full-Unicode `to_uppercase`, slices on character
boundaries, and pushes/pops characters. lsaca takes 27.9 ms versus 10.0 ms for C and
11.7 ms for Rust (2.78x C, 2.38x Rust). The C program walks UTF-8 bytes directly; the
lsaca program goes through the runtime's UTF-8-aware `String` methods and the
character-lookup tables, so the gap is the price of the validated-string API rather than
of the loop. Peak memory is 5.4 MiB versus 4.3 MiB for C. Python takes 222.1 ms and
Node.js 158.3 ms, both far slower than the native implementations.

### Compile times: direct C back end, not CMake

A cold lsaca build of a small program is 0.91-1.22 s in this session (0.74-1.05 s in the
1.5 session, 1.03-1.32 s in the 1.4 session, 1.15-1.72 s in the 1.3 session, 0.97-1.37 s
in the 1.2.0 session). Two things dominate it: compiling the C runtime translation units
(about 300 KB of C in six files, now including the Unicode tables and the Swiss-table
map) and compiling plus linking the generated program units; a single-file `gcc -O2`
compile of the same program takes 0.21-0.29 s, so the runtime and the project overhead
account for most of the difference. A cached build is 14-21 ms because the
content-addressed cache returns the previous executable without invoking gcc at all.

In 1.1.0 the same cold build took 2.79-3.27 s because the pipeline included CMake
configuration and a Ninja build of the generated project. The 1.2 direct C compiler back
end (the default) skipped that layer, which made cold builds about 2.6-3.3x faster. The
old pipeline is still available with `--cmake` for comparison and for environments where
direct gcc invocation is not possible.

`compile_large` shows the scaling in generated-C size: 395,372 bytes (about 0.40 MB) of
program C in `main.c` plus three `lsaca_unit_N.c` files, 1.83 s cold, 244 ms cached -
roughly 2x a small program's cold build. The 1.5 session emitted the same program as one
1,046,885-byte `main.c` and took 3.14 s, so splitting the C across units and shrinking
the code made the large build about 1.7x faster.

The answer to "lsaca compile speed" therefore depends on the cache: a first build is
slower than gcc by roughly 3-4x for small programs (mostly gcc on the runtime);
rebuilding an *unchanged* program is faster than either compiler, because the cache hit
skips gcc entirely.

### What 1.6 changed: characters, a new map and a faster tree

1.6 added the `char` scalar type with character literals, made every `String` a
guaranteed-valid UTF-8 string with character-aware methods (`char_count`, `chars`,
`char_at`, `push`/`pop`, `String::from_char`), extended
`to_uppercase`/`to_lowercase`/`trim` to full Unicode, rewrote `HashMap` as a
Swiss-Table-style map with `char` keys, and fixed a batch of runtime and code-generation
issues (stale-handle checks, iterative ORC cycle collection, buffered I/O, shortest
Dragon4 float formatting, interned NUL-safe string literals). Three new workloads measure
the new surfaces, each against the closest idiomatic C/Rust equivalent:

| workload | lsaca (ms) | C (ms) | Rust (ms) | lsaca vs C | what it isolates |
| --- | ---: | ---: | ---: | ---: | --- |
| `string_unicode` | 27.9 | 10.0 | 11.7 | 2.78x | UTF-8 string API and Unicode case mapping |
| `hashmap_i64` | 211.3 | 133.0 | 172.1 | 1.59x | integer-key map operations |
| `hashmap_string` | 241.7 | 165.8 | 246.8 | 1.46x | owned `String` keys, hashing and deletion |

The ownership rows improved against the 1.5 session even though the source did not
change: the by-value tree traversal moved from 534.4 ms / 101.2 MiB to 322.4 ms /
52.3 MiB (40% less time, 48% less peak memory) after match arms learned to steal owned
bindings instead of copying the matched value, and the borrowed traversal moved from
34.1 ms / 19.9 MiB to 26.8 ms / 15.8 MiB. The rest of the suite moved within the
session-to-session range already discussed (this clean run is not uniformly faster or
slower than the 1.5 session: the scalar loops drifted slightly slower, `vec_memory`
faster); the new character and map workloads are the only rows that changed the shape of
the table.

### Python and Node.js: start-up, interpretation and JIT

Python is 4.46-102.66x slower than C on these workloads. The widest gap is `loop_sum`
(912.2 vs 8.9 ms, 102.66x), where CPython executes 8 million integer additions in a
bytecode loop; next is `matrix_multiply` (1149.7 ms vs 12.6 ms, 91.40x), where 4.1 million
multiply-adds run in nested loops; the narrowest is `fn_pipeline` (32.5 vs 7.2 ms,
4.52x), then `binary_trees_refs` (5.33x) and `string_split` (5.49x), where most of the
work happens in C-level methods or on object references. Python is faster than lsaca
only on the by-value `binary_trees` row (86.1 vs 322.4 ms), for the aliasing reason
explained above.

Node.js lands between: 2.23-18.42x C. V8's JIT does well on the simple loops
(`sort_i64` 162.2 ms, 4.79x; `mandelbrot` 127.0 ms, 5.24x; `sieve` 142.0 ms, 5.06x) and
even comes close to C on the string-key map (`hashmap_string` 370.4 ms, 2.23x), but the
numbers include V8 start-up on every run (each measured run is a fresh process), and its
memory floor is visible in the memory table (40.3-121.4 MiB for the workloads shown
versus 4 MiB for native code).

Interpreted-language numbers in this table are one-shot script invocations, not a warm
long-running server; that is the fair comparison for "run a small program once".

## Fairness notes and caveats

1. **Idiomatic, not identical, programs.** The suite measures each language as its users
   would write it, and that includes representation choices:

   | workload | lsaca | C | Rust | Python | Node.js |
   | --- | --- | --- | --- | --- | --- |
   | `string_build` | `String::new()` + `push_str(format!(...))` | one `malloc`ed char buffer of exact size | `String::with_capacity` + `to_string` + `push` | list of `str` + `",".join` | array + `join` |
   | `vec_memory` | `Vec<Point>` + `with_capacity` + `push` | `malloc`ed array of structs | `Vec<Point>` with `with_capacity` | list of tuples | array of objects |
   | `sort_i64` | iterative quicksort on `Vec<i64>` | same quicksort on a `malloc`ed array | `Vec<i64>` + `swap` | list + swapping | array + swapping |
   | `binary_trees` | `Option<Box<Node>>` recursive struct; the by-value traversal clones child subtrees with `.clone()` (moving a field out is `E0507`) | `malloc`ed nodes + raw pointers | `Option<Box<Node>>` if let / match | class with `__slots__` | plain objects |
   | `binary_trees_refs` | same tree, summed via `&Option<Box<Node>>` (no copy) | same program as `binary_trees` (C uses pointers anyway) | same program as `binary_trees` (moving a `Box` is a pointer move) | same program | same program |
   | `sieve` | `vec![0; limit + 1]` of `i64` | `calloc`ed `int64_t` array | `vec![0i64; n]` | list of ints | `Float64Array` |
   | `iterator_pipeline` | `values.iter().filter(...).map(...).fold(...)` lazy adapter structs from the prelude | eager `for` loop with the same arithmetic | its own iterator chain | generator expression inside `sum` | eager loop with `BigInt` squaring |
   | `hashmap_wordcount` | builtin `HashMap<String, i64>` with `get`/`insert`/`keys` | hand-written open-addressing map with `malloc`ed `char *` keys | `std::collections::HashMap` + `entry` | `dict` | `Map` |
   | `generic_stack` | one generic `Stack<T>`, monomorphized twice | two hand-written typed stack implementations | one generic `Stack<T>` | one class `Stack` | one class `Stack` |
   | `fn_pipeline` | `Rule { label: String, apply: fn(i64) -> i64 }` + `.clone()` of a rule | struct with `const char *` and a function pointer | struct with `String` and `fn(i64) -> i64` | class holding a lambda | object holding a function |
   | `string_unicode` | `String` built one `char` at a time; `.chars()`, `char_at`, `to_uppercase`, boundary-checked `substring` | `char *` buffer with byte-oriented UTF-8 handling | `String` with `char` iteration and `to_uppercase` | code-point list + `str.upper` | code-point array + `toUpperCase` |
   | `hashmap_i64` | builtin `HashMap<i64, i64>` with `insert`/`get`/`remove` | hand-written open-addressing map with `int64_t` keys | `std::collections::HashMap<i64, i64>` | `dict` | `Map` |
   | `hashmap_string` | builtin `HashMap<String, i64>` with owned keys | hand-written open-addressing map with `strdup`ed keys | `std::collections::HashMap<String, i64>` | `dict` | `Map` |

   These are all reasonable code, but they do different amounts of low-level work, and
   that is exactly what the benchmarks are measuring.

2. **Contiguous values versus per-element objects.** C, Rust and lsaca store `Point`
   values contiguously in the vector/array; Python tuples and JS objects are individual
   heap objects referenced from the list/array. The memory rows for `vec_memory` are
   largely a consequence of that representation difference, not only of the runtime. The
   same inversion explains why Python (86.1 ms) beats lsaca on `binary_trees`
   (322.4 ms): reference semantics make aliasing cheap, while lsaca's by-value traversal
   copies subtrees. On the borrowed traversal the order reverses - lsaca 26.8 ms versus
   Python 85.6 ms - because not copying beats cheap aliasing.

3. **Value semantics: moves, not implicit copies.** An lsaca `Vec`, `Box`, `String` or
   `HashMap` is a move-only value: moving it (assignment, passing it by value, returning
   it) moves the payload, and `.clone()` is the explicit way to copy it. That is closer
   to Rust's move model than to the implicit copying of 1.3, but the reference checker is
   conservative: `&T`/`&mut T` are non-owning pointers with an alias check, not
   lifetime-tracked borrows, and storing a reference in an aggregate is still rejected.
   References are how `binary_trees_refs` traverses the tree without copying it; the
   by-value `binary_trees` variant has to clone each child subtree explicitly, and that
   difference is the measured value of reference support. Earlier versions avoided copies
   where they could - `Vec::with_capacity(n)` bound to a `let`, and `for x in vec` without
   duplicating the buffer (hence the `vec_memory` parity) - and later versions keep those
   by moving instead of deep-copying.

4. **What the current language does and does not have.** At measurement time lsaca has
   generics, user traits (static dispatch only), iterators, `HashMap`, the `char` type,
   UTF-8-aware strings, and ownership with auto-derived `Copy`/`Clone`/`Drop`. It still
   has no trait objects (no `Box<dyn Trait>`), no library sort method, no unsigned or
   small integer types used in these programs, and no operator overloading beyond the
   builtin `==`. The workloads stay deliberately simple for that reason: `sort_i64` does
   not call a library sort in any language, and the string workloads use only the
   documented `String`/character API. `iterator_pipeline` compares the documented
   iterator surface against a hand-written loop rather than against an optimising
   iterator library. These numbers describe the 1.6.0 implementation, not the language's
   potential.

5. **What the lsaca release build does.** Release builds wrap `+ - *` (the generated C
   uses explicit unsigned casts), so there are no overflow checks on those operations.
   Division and remainder always go through the runtime's checked helpers because
   division by zero panics even in release; `string_build` performs one million
   `i % 1000` operations, `sort_i64` 200,000 LCG remainders and `matrix_multiply`'s fill
   loop uses `%` as well. Vector indexing is bounds-checked on every access, which is the
   main extra work in `matrix_multiply`'s inner loop; that row comes out at parity with C
   this run (0.99x, as it did in the 1.5 session), so at this 16 ms size the checks are
   lost in code-generation and session differences. The runtime's
   reference-count and copy-on-write checks are per string call. String literals are
   interned once per distinct literal, so literal data in these programs is not
   reallocated per evaluation; moving a value is a pointer move with no allocation, a `.clone()` performs exactly the deep copy the source
   asks for, and `Drop` hooks run when a heap value is released - so ownership adds no
   per-value bookkeeping to these loops beyond the reference count the runtime already
   maintained. The iterator and `HashMap` rows are where the abstraction cost shows: each
   adapter stage is a function call on the generated C, and each map operation retains or
   releases owned keys and values.

6. **Compiler and flags.** gcc 15.2.0 (MSYS2 UCRT64) and rustc 1.99.0, each with the
   single normal optimisation flag (`-O2` / `-O`). No LTO, PGO, `-march=native` or
   target-cpu tuning for any language. The lsaca runtime is compiled by the same gcc with
   `-O2 -DNDEBUG`. The comparison is of standard builds on one Windows machine, not of
   tuned builds on Linux.

7. **Noise, statistics and a shared machine.** 7 measured runs, median (and minimum)
   reported. This is a laptop with P/E cores, power management and background activity.
   The published run was taken on an otherwise idle machine; an earlier 1.6 attempt that
   overlapped other workloads was discarded because its unchanged C/Rust/Python/Node
   baseline rows came out 17-35% slower. Treat single-digit percentage differences as
   noise, do not compare absolute numbers across pages or sessions, and prefer
   comparisons made inside one run. Session-level effects dominate absolute numbers: this
   run and the 1.5 session disagree in both directions (C's `loop_sum` went from 9.2 to
   8.9 ms while lsaca's went from 8.1 to 9.6 ms, and both got faster on `vec_memory`),
   `rustc -O` took 0.21-0.56 s per file versus 0.93-1.65 s in the 1.3 session, and gcc
   took 0.21-0.29 s. The medians absorb individual bad samples: the 1.3 run contained one
   Python `sieve` sample at 2.48 s against a 772.5 ms median, and the 1.2.0 run one
   Python `string_build` sample at 5.38 s; median and minimum are unaffected. The 1.3
   session also discarded a first attempt that was 1.2-1.6x slower across every language,
   C and Rust included - a uniform slowdown means a busy machine, not a slow compiler.
   This 1.6 run recorded no warnings: no sample needed re-measuring.

8. **Peak memory is working set, not allocation.** It includes the executable, runtime
   metadata and allocator arena. For Node.js the floor is 40-121 MiB and for Python
   ~11 MiB, so their entries measure the interpreter as much as the data. Sampling can
   miss very short spikes.

9. **Start-up is included in every runtime number.** Each measured run is a new process:
   dynamic loading, runtime initialisation and (for Python/Node.js) interpreter/VM start-up
   are all part of the time. For a ~11 ms workload like `loop_sum` in C, start-up is a
   noticeable fraction of the number.

10. **Compile-time columns answer different questions.** Cold lsaca includes C emission,
    compiling the six runtime translation units, compiling the generated C and linking;
    cached lsaca is a cache hit; gcc/rustc are bare single-file compiles; the
    "emit-c + gcc" column is only used in fallback mode. `compile_large` is compile-only
    and never executed. Do not compare cross-column compile numbers as if they were the
    same pipeline.

11. **Default back end versus `--cmake` versus fallback.** The numbers on this page use
    the default `lsaca build -r` path (direct C compiler back end). The older CMake +
    Ninja path still exists via `--cmake` and is not measured here; it was the 1.1.0
    default and was about 2.6-3.3x slower for a cold build. In environments where the
    default path cannot be used (for example a sandbox that blocks nested child
    processes), the harness can fall back to `lsaca emit-c -r` plus a direct `gcc -O2`,
    which the harness labels as fallback in `latest.json`; those numbers are not mixed
    into the tables here.

12. **Checksum validation checks output, not identical work.** The checksums guarantee the
    programs compute the intended result; they do not force the allocation patterns to be
    equal. The idiomatic differences in point 1 remain part of the measurement.

13. **One machine, one session.** Absolute numbers will differ on other CPUs, operating
    systems and toolchain versions. The raw run arrays in `latest.json` are the primary
    record; this page is a snapshot, not a running leaderboard.

## Reproducing

Requirements:

- Python 3 (standard library only - the harness has no third-party dependencies).
- On Windows, gcc for the lsaca default build path. Since 1.2 CMake and Ninja are no
  longer needed for a normal build; they are only required if you pass `--cmake`
  yourself. The harness finds gcc via `PATH` and the usual MSYS2 install locations.
- Rust (for the Rust row), Node.js (for the Node.js row). Missing tools are reported and
  the corresponding rows are skipped rather than failing the run.

From the repository root:

```powershell
# 1. Build the compiler once (the harness copies target/release/lsaca.exe and reuses it).
cargo build --release -p lsaca-cli

# 2. Run the full suite (a few minutes, depending on the machine).
python scripts/bench.py
```

The suite writes:

- `benchmarks/results/latest.json` - machine info, toolchain versions, settings, the
  expected stdout for each workload and every individual compile/runtime/memory sample;
- `benchmarks/results/latest.md` - the tables in the same shape as this page;
- a markdown summary on stdout.

Useful options:

```powershell
python scripts/bench.py --quick                    # ~1 minute: 3 runtime runs, 1 compile repeat
python scripts/bench.py --programs fib,sort_i64    # only some workloads
python scripts/bench.py --languages lsaca,c        # only some languages
python scripts/bench.py --list                     # list workload/language names
python scripts/bench.py --build-mode official      # the mode used for the numbers on this page (default)
python scripts/bench.py --build-mode fallback      # emit-c -r + gcc instead of the default path (labelled)
python scripts/bench.py --no-memory                # skip the peak-memory measurements
python scripts/bench.py --help                     # all options
```

The harness exits non-zero if any measured run produces the wrong output or a non-zero
exit code, so a broken implementation fails the run instead of producing numbers. Run the
suite on an otherwise idle machine; if you compare with the numbers above, compare the
same build mode (official) and keep in mind the caveats in the previous section. Benchmark
sources and harness internals are described in
[benchmarks/README.md](../../benchmarks/README.md).

## See also

- [benchmarks/README.md](../../benchmarks/README.md) - harness usage, output layout, how
  to add a workload.
- [benchmarks/results/latest.json](../../benchmarks/results/latest.json) - the raw data
  behind this page.
- [lsaca documentation](../README.md) - the manual hub and full index.
- [Getting started](01-getting-started.md) - the first chapter of the language tour.
