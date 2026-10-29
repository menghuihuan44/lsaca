# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
intuition: the same fifteen workloads are implemented in lsaca, C, Rust, Python and
Node.js, built with each language's normal toolchain, run on the same machine, and checked
against a shared expected output. A sixteenth, larger source is measured for compile time
only.

The numbers below were measured on **2026-10-29** with the repository's harness,
`scripts/bench.py`, on the reference machine described in
[Environment](#environment). The raw data (including every individual run, not just the
medians) is in [benchmarks/results/latest.json](../../benchmarks/results/latest.json);
the harness also writes a machine-generated subset of these tables to
[benchmarks/results/latest.md](../../benchmarks/results/latest.md). Instructions for
reproducing the run are at the end of this page.

The short version for lsaca 1.5.0:

- **Scalar loops, recursion and floating-point code are in the same ballpark as C.**
  `loop_sum` is 8.1 ms in lsaca versus 9.2 ms in C, `fib` 20.4 versus 21.5 ms,
  `sieve` 24.0 versus 22.9 ms, `mandelbrot` 26.4 versus 27.6 ms and `fn_pipeline`
  9.9 versus 11.0 ms. These are all sub-100 ms workloads, where session-to-session noise
  is comparable to the gaps themselves (see the noise caveats and the per-workload
  analysis); read the rows as "same range as C", not as stable speed ratios.
- **`vec_memory` is within 1.2x of C**: 18.6 ms and 18.8 MiB peak versus C's 15.9 ms /
  18.8 MiB and Rust's 14.5 ms / 18.9 MiB (Rust was faster on this row this session; all
  three peaks are equal). In 1.1.0 the same
  workload took 86.1 ms and 35.1 MiB, so the runtime and code-generation work since then
  still holds.
- **`string_build` is 2.1x faster than Rust here** (27.0 ms versus Rust's 56.2 ms) but
  still 0.37x slower than C (10.0 ms). The remaining cost is the runtime's per-token
  string path, not the generated C; cross-session ratios move because C's single-buffer
  version also got faster this session (see the analysis).
- **1.4 ownership and the 1.5 collections keep the cost of copying explicit.**
  `binary_trees_refs` sums the same tree through `&Option<Box<Node>>` and copies
  nothing: 34.1 ms and 19.9 MiB, which is 2.25x C's runtime and 2.6x its peak memory.
  The value-taking `binary_trees` variant has to `.clone()` each child subtree; it pays
  534.4 ms and 101.2 MiB - 15.7x slower and 5.1x more peak memory than the borrowed
  traversal. That gap is the measured cost of value semantics used naively.
- **1.5 generics, iterators and HashMap are usable and measured.** A generic `Stack<T>`
  monomorphizes to code that stays in the range of a hand-written C stack (14.2 versus
  17.4 ms); a struct carrying a function value matches C function pointers (9.9 versus
  11.0 ms); a lazy `filter/map/fold` chain costs 2.32x a hand-written C loop (27.8 versus
  12.0 ms) but allocates nothing extra (11.2 MiB peak, same as the loop); a
  `HashMap<String, i64>` word count is 1.91x a hand-written open-addressing C map (46.4
  versus 24.3 ms) and 1.07x Rust's `std::collections::HashMap` (43.4 ms).
- **A cold build takes 0.74-1.05 s** for the small programs; the default direct C
  compiler back end emits C and drives gcc itself. A cached build is 13-20 ms. The
  4,511-line `compile_large` source (1.05 MB of generated C) takes 3.14 s cold and
  227 ms cached.
- **Interpreters are 3.26-93.05x slower than C** on these workloads; their numbers
  also carry interpreter/VM start-up on every run.

## Workloads

Fifteen small programs, each of which prints a single `checksum=...` line, plus a large
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
| `compile_large` | compile-time scaling with source size | generated 4,511-line lsaca source (450 functions); **compile-only**, never executed by the harness | `checksum=14655037` (not checked) |

`iterator_pipeline`, `hashmap_wordcount`, `generic_stack` and `fn_pipeline` are new in
1.5.0; the other eleven runtime workloads carry over from the earlier releases, so
the 1.4 session figures quoted in the analysis below are directly comparable.

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
  compile the generated `main.c`, compile the five runtime translation units, and link,
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

All 76 (workload, language) combinations were built (75 of them also run), and every
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
| Measured at | 2026-10-29 21:36:59 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 1.5.0, repository HEAD `e7c45ca`, compiler sha256 prefix `fb83100f8d6cf92e`, compiler mtime 2026-10-29 21:30:37 +08:00 |
| lsaca build path | `lsaca build -r`, default direct C compiler back end (gcc 15.2.0, `-O2 -DNDEBUG`); the old CMake + Ninja path is available as `--cmake` |
| C compiler | gcc 15.2.0 (MSYS2 UCRT64, Rev14, Built by MSYS2 project) |
| Rust compiler | rustc 1.99.0 (`b940084d7 2026-09-28`) |
| Python | 3.14.4 |
| Node.js | v24.14.1 |
| Harness | `scripts/bench.py`, 16 programs (15 executed), 1 warm-up + 7 runs, 3 compile repeats, peak-memory sampling on |

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

The "lsaca emit-c + gcc" column is only filled when the harness runs in fallback mode
(see the fairness notes); in this official run the default build back end was used and
that column is not measured.

Observations:

- A cold lsaca build is **0.74-1.05 s** for the fifteen runtime workloads (the fastest
  is `string_build` at 0.741 s, the slowest `vec_memory` at 1.047 s). The previous sessions measured
  1.03-1.32 s for the 1.4 suite and 1.15-1.72 s for the 1.3 suite. Most of the cold cost
  is gcc: compiling the generated `main.c` together with the five runtime translation
  units and linking them (a single-file gcc compile of the same workload is 0.19-0.30 s). The front end itself is a small part of the measurement -
  the C emission step is not separately recorded but is a few tens of milliseconds.
- `compile_large` shows the scaling: 4,511 lines / 450 functions produce 1,046,885
  bytes (about 1.05 MB) of generated C and 3.14 s of cold build time, roughly 3-4x a
  small program's build. The build time therefore grows with the generated C, as
  expected for "run gcc on C".
- A cached lsaca build is **13-20 ms** for the small programs (227 ms for
  `compile_large`). That is the content-addressed cache hit: the compiler hashes the
  generated program and returns the existing executable rather than invoking gcc. In
  practice an edit to the `.ls` source invalidates the cache and pays the full cold cost
  again; there is no incremental compile yet.
- The C column (0.19-0.30 s) is a single-file compile with no project setup; it is the
  right baseline for "front end plus codegen", not for the whole emit-compile-link
  pipeline. In this session `rustc -O` took 0.20-0.39 s, comparable to gcc; earlier
  sessions on the same machine measured 0.93-1.65 s for the same rustc invocations (see
  the caveats), which is a reminder that absolute compile times here are sensitive to
  the machine's state, not to these benchmark sources.

### Runtime

Milliseconds, **median (minimum)** of 7 measured runs after 1 warm-up. Lower is better.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 20.4 (18.8) | 8.1 (7.6) | 27.0 (24.8) | 18.6 (15.5) | 28.0 (26.7) | 12.8 (11.6) | 534.4 (519.3) | 34.1 (30.9) | 15.7 (14.4) | 24.0 (23.1) | 26.4 (23.8) | 27.8 (27.5) | 46.4 (42.1) | 14.2 (13.6) | 9.9 (9.0) |
| C (gcc -O2) | 21.5 (19.2) | 9.2 (7.6) | 10.0 (9.7) | 15.9 (15.3) | 29.8 (25.6) | 12.7 (12.2) | 17.7 (17.3) | 15.1 (14.4) | 11.9 (10.7) | 22.9 (22.4) | 27.6 (26.9) | 12.0 (11.0) | 24.3 (21.9) | 17.4 (16.4) | 11.0 (10.7) |
| Rust (rustc -O) | 10.2 (9.5) | 10.0 (9.4) | 56.2 (54.0) | 14.5 (14.0) | 24.7 (23.7) | 9.0 (8.9) | 19.4 (18.5) | 17.7 (17.0) | 11.4 (11.1) | 23.9 (22.9) | 26.9 (25.6) | 14.5 (11.9) | 43.4 (39.8) | 16.1 (14.8) | 9.0 (8.5) |
| Python 3 | 810.6 (749.6) | 805.6 (726.3) | 370.6 (326.7) | 417.4 (397.4) | 574.9 (549.3) | 1179.3 (1081.7) | 97.4 (92.2) | 83.4 (79.0) | 55.9 (52.8) | 474.3 (439.7) | 2138.4 (1997.9) | 213.1 (211.2) | 101.2 (88.1) | 313.0 (295.9) | 35.8 (33.4) |
| Node.js | 190.0 (176.2) | 101.2 (96.3) | 250.5 (223.6) | 195.8 (178.9) | 142.7 (129.3) | 106.2 (103.2) | 110.9 (105.8) | 107.5 (106.4) | 117.2 (113.6) | 157.2 (137.7) | 119.8 (117.6) | 138.1 (130.5) | 118.3 (111.0) | 126.4 (123.1) | 113.7 (107.7) |

The same numbers as a ratio to the C time (median / C median). Below 1.0 means faster
than C:

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 0.95x | 0.89x | 2.71x | 1.17x | 0.94x | 1.01x | 30.11x | 2.25x | 1.32x | 1.05x | 0.96x | 2.32x | 1.91x | 0.82x | 0.91x |
| Rust | 0.47x | 1.09x | 5.63x | 0.91x | 0.83x | 0.71x | 1.09x | 1.17x | 0.96x | 1.04x | 0.98x | 1.21x | 1.79x | 0.93x | 0.82x |
| Python 3 | 37.65x | 87.55x | 37.15x | 26.18x | 19.27x | 93.05x | 5.49x | 5.52x | 4.70x | 20.69x | 77.50x | 17.78x | 4.17x | 18.03x | 3.26x |
| Node.js | 8.83x | 11.00x | 25.11x | 12.28x | 4.78x | 8.38x | 6.25x | 7.11x | 9.86x | 6.86x | 4.34x | 11.52x | 4.87x | 7.28x | 10.36x |

### Peak memory

MiB of peak working set, separate run. Lower is better. Remember that this is total
process footprint: it includes the runtime, the interpreter/VM and its heap.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot | iterator_pipeline | hashmap_wordcount | generic_stack | fn_pipeline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 18.8 | 6.6 | 4.4 | 101.2 | 19.9 | 11.1 | 18.9 | 4.0 | 11.2 | 4.3 | 12.6 | 4.1 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.6 | 7.7 | 7.7 | 4.1 | 18.9 | 4.0 | 11.2 | 4.3 | 12.6 | 4.0 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 | 11.2 | 4.4 | 12.7 | 4.2 |
| Python 3 | 11.0 | 10.9 | 73.6 | 142.8 | 21.1 | 13.6 | 21.0 | 21.0 | 16.9 | 26.2 | 11.0 | 49.3 | 11.4 | 54.3 | 11.1 |
| Node.js | 43.0 | 45.7 | 89.3 | 121.3 | 50.2 | 47.3 | 56.0 | 56.1 | 51.1 | 60.3 | 44.7 | 58.1 | 46.4 | 72.4 | 40.2 |

For workloads with no large data structures (`fib`, `loop_sum`, `mandelbrot`,
`fn_pipeline`) every native language sits at the 4 MiB floor of the process image, and
`generic_stack` stays at 12.6-12.7 MiB for the three native languages. Where the data is
large: `vec_memory` allocates 16 MB of `Point` values and both lsaca and C peak at
18.8 MiB; `sieve` keeps a 2,000,000-element `i64` flag vector, 16 MB, and peaks at
18.9 MiB; `string_build` allocates a ~3.9 MB string and adds a temporary per token,
hence 10.6 MiB versus 7.3 MiB for C. `string_split` peaks at 11.1 MiB versus 4.1 MiB
for C because the split tokens and the parsed values are heap `String`s on the lsaca
side. `binary_trees` is the outlier at 101.2 MiB for 131,071 small nodes, because the
by-value traversal clones whole subtrees; the borrowed `binary_trees_refs` traversal
copies nothing and peaks at 19.9 MiB, 2.6x C's 7.7 MiB instead of 13.2x.

## Analysis

### Scalar integer loops: parity with C

`loop_sum` is the cleanest comparison: 8,000,000 iterations of `sum += i` and nothing
else. lsaca takes 8.1 ms and C 9.2 ms - within about 10% - while Rust measured 10.0 ms on
the same program this session. In release mode the generated C lowers integer
additions to plain C arithmetic (the wrapping semantics are expressed as explicit
`uint64_t` casts, so the compiler does not need to keep overflow checks), and GCC -O2
turns the loop into comparable machine code to a hand-written C loop.

`sieve` (24.0 vs 22.9 ms) and `mandelbrot` (26.4 vs 27.6 ms) tell the same story for an
array-heavy loop and for floating-point arithmetic: when the work does not need the
runtime's help, lsaca generates C that performs like C. In this session lsaca came out
slightly ahead on `mandelbrot` and slightly behind on `sieve`; on 20-30 ms workloads that is GCC-vs-LLVM
code-generation noise, not evidence that generated C is faster than C. This is the
strongest evidence that the C backend is not intrinsically slow.

### Recursion-heavy calls: fib

The recorded session has lsaca at 20.4 ms, C at 21.5 ms and Rust at 10.2 ms for the same
recursive algorithm. Two things matter here, and only one of them is about lsaca:

1. **C and Rust differ by 2.1x on identical source-level algorithms.** That is a
   code-generation effect between GCC 15.2 and LLVM (via rustc 1.99.0) on a
   self-recursive function, not a language-semantics effect. Any single workload can
   show a compiler gap of this size; it should not be read as "Rust is faster than C".
2. **The lsaca / C difference moves between sessions.** In this session lsaca came out at
   0.95x C (slightly ahead); in the 1.4 session it was 0.82x C, in the 1.3 session 1.24x
   C, and an earlier interleaved re-timing of a previous binary pair (9 runs each,
   alternating) measured 36.9 ms (lsaca) vs 39.0 ms (C) - roughly parity. Treat the fib
   row as "same ballpark", not as a stable speed ratio.

The constant argument n=30 does not explain the differences: probes with
`std::hint::black_box(30)` in Rust and a `volatile` n in C produced timings within a few
percent of the plain versions, so the compilers are not constant-folding the recursion. In
the generated lsaca program n is a runtime variable anyway.

### String building: runtime-bound, 2.7x C

`string_build` appends 1,000,000 small strings. lsaca needs 27.0 ms versus 10.0 ms for C
and 56.2 ms for Rust (Rust is slower than C here because it also allocates a temporary
`String` per token). In lsaca 1.1.0 the same program took 212.9 ms; the 1.2 runtime work
brought that down to 33.9 ms, and this session measures 27.0 ms - 7.9x faster than 1.1.0
and 2.1x faster than the Rust version. C measured 16.7 ms in the 1.4 session and 10.0 ms
here, so the ratio to C moved from 2.4x to 2.71x even though the lsaca number improved:
compare within a row, not across sessions.

The remaining gap to C is about what a `String` is in each implementation:

- The C program allocates one buffer of the exact final size up front and writes bytes
  into it. There are almost no function calls in the hot loop.
- At the lsaca level the program is `s.push_str(format!("{},", v))`. Each iteration
  builds a temporary formatted `String` and appends it through the runtime's string
  append function, which handles reference counting and geometric growth. The loop
  therefore performs one allocation plus one append per token; C's version performs none.
- The peak-memory row reflects the same design: the last growth step keeps the old and
  the new buffer alive at the same time (10.6 MiB total versus 7.3 MiB for C and Rust).

The measured gap is a statement about the current runtime implementation, not about the
language: an API that let the caller append into a pre-sized buffer, or that inlined the
growth path, could close most of it.

### Vectors of struct values: within 1.2x of C

`vec_memory` pushes 1,000,000 two-word structs into a `Vec`. In this session lsaca takes
18.6 ms and peaks at 18.8 MiB, versus 15.9 ms / 18.8 MiB in C and 14.5 ms / 18.9 MiB in Rust
(Rust was faster on this row this session, and all three peaks are equal). In 1.1.0 it
was 86.1 ms and 35.1 MiB.

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
through bounds-checked indexing. lsaca takes 28.0 ms versus 29.8 ms in C and 24.7 ms in
Rust, 0.94x C - slightly ahead, well within session noise. The C version uses the same
algorithm and the same explicit stack; the Rust version uses `swap` on a mutable slice
and was notably faster this session (0.83x C), which is why the Rust row is not a useful
baseline here.

No language uses a library sort in this workload (lsaca has no `Vec::sort`), so this is a
fair "how expensive are the language's primitives in a write-heavy loop" comparison, and
the answer is: in the same range as C. The swap-and-index loop executes bounds checks in
lsaca that C does not, but this workload is branch-heavy enough that its absolute time
moves more between sessions than that overhead; treat the row as "same range", not as a
measured constant. A library `qsort`/`sort_unstable` in C or Rust would be faster than
any of these numbers.

### Matrix multiplication: bounds checks in the inner loop

`matrix_multiply` multiplies two 160x160 `i64` matrices stored as flat vectors. lsaca
takes 12.8 ms versus 12.7 ms for C and 9.0 ms for Rust - parity in this session. The
inner loop indexes the two input vectors and the output vector by computed positions
(`a[i*n+k] * b[k*n+j]`), so every access is bounds-checked in lsaca, while C and Rust
index without a check. Bounds checks of that shape are typically a few percent on a
workload this small; the session-to-session swing (and the fact that the whole program is
about 11 ms) means this row is dominated by code-generation and session noise. The
arithmetic itself is plain C code, which is why all three implementations land in the
same range.

### Binary trees: value semantics versus the borrowed traversal

`binary_trees` builds a perfect tree of 131,071 nodes and sums every payload through a
value-taking function (`fn sum_tree(tree: Option<Box<Node>>) -> i64`). C takes 17.7 ms
and peaks at 7.7 MiB; lsaca takes 534.4 ms and peaks at 101.2 MiB. This is by far the
largest gap in the suite, and it is a language-model effect rather than a codegen effect:

- In C the tree is `malloc`ed nodes with raw pointers: building and traversing it never
  duplicates anything.
- In Rust the tree is `Box<Node>`: moving a `Box` is moving a pointer.
- In lsaca, move-only values are moved, so building the tree and passing it into
  functions does not duplicate it. But the traversal owns each node it matches and then
  needs a copy of its children: `Box::get()` returns an owned copy of the payload, and
  taking a child by value would move it out of a field, which is `E0507`, so the program
  clones the child subtrees explicitly with `.clone()`. Every level of the recursion
  therefore copies its subtree. The 30x runtime and the 13.2x peak-memory gaps are the
  price of that value-semantics shape, and the source shows the copies instead of hiding
  them in implicit copies.

The second workload uses exactly the same tree: `binary_trees_refs` sums it through
references instead - `fn sum_tree(tree: &Option<Box<Node>>) -> i64`, matching
`Some(boxed)` and recursing on `&current.left` and `&current.right`. That traversal does
not copy the tree at all. It takes 34.1 ms and peaks at 19.9 MiB, so **the borrowed
traversal is 15.7x faster and uses 5.1x less peak memory than the value traversal on the
same tree, same nodes and same algorithm**. It is also 2.25x C's runtime and 2.6x its
peak memory - the same order as the rest of the suite rather than a factor of ten.

The borrowed version is not free of the runtime: it is still 2.6x C's peak memory, because
the tree is built and dropped out of reference-counted `Box` nodes. It does beat the
interpreters, though: Python takes 83.4 ms and Node.js 107.5 ms on the same borrowed
traversal, versus lsaca's 34.1 ms. Python's 97.4 ms on the by-value variant is still
much faster than lsaca's 534.4 ms, because Python objects are references and aliasing is
the default there.

### Iterators: a lazy pipeline against a loop

`iterator_pipeline` is the first workload that measures the 1.5 iterator abstraction
directly. It builds 1,000,000 `i64` values and then runs
`values.iter().filter(|x| *x % 2 == 0).map(|x| x * x).fold(0, |acc, x| acc + x)`.
The C version runs the same arithmetic as an eager `for` loop, Rust uses its iterator
chain, Python a generator expression and Node.js an eager loop.

lsaca takes 27.8 ms versus 12.0 ms for the C loop and 14.5 ms for Rust - the chain costs
2.32x a hand-written loop in the current implementation. That is the overhead of the
abstraction, not of allocation: the chain is lazy, so no intermediate vectors are
materialised, and its peak memory is 11.2 MiB, exactly the same as the eager C loop. In
the generated C each adapter is a generic struct with its own `next` function, so every
element passes through a call per adapter stage (filter, map) instead of one loop body;
Rust's optimizer inlines its equivalent chain completely, which is why Rust is not paying
that cost. The result is still 7.7x faster than Python (213.1 ms) and 5.0x faster than
Node.js (138.1 ms). A hand-written loop in lsaca would land close to C, as `loop_sum`
shows: the gap here is the price of the lazy chain, and it is the current implementation,
not a fundamental property of the language.

### HashMap: a builtin map against a hand-written one

`hashmap_wordcount` counts 200,000 deterministic words drawn from a 4,096-word alphabet,
then folds the squared counts. The C version is a hand-written open-addressing map with
`malloc`ed `char *` keys; Rust uses `std::collections::HashMap` with `String` keys;
Python uses a dict and Node.js a `Map`.

lsaca takes 46.4 ms versus 24.3 ms for the C map (1.91x) and 43.4 ms for Rust's
`std::collections::HashMap` (1.07x slower than Rust). It beats Python's 101.2 ms by 2.2x
and Node's 118.3 ms by 2.6x, and its peak memory is 4.3 MiB - the same as the C map's
4.3 MiB. The gap to C is the price of ownership in the map interface: keys are move-only
`String`s, `get(key)` copies the key argument and clones the stored value, `insert`
moves the key and value in, and the checksum loop calls `keys()` (which clones every key
into a `Vec<String>`) and then `get` again for each key. Each of those operations is an
ORC retain/release or a small clone on a short string, where the C map passes a
`char *` and never touches a reference count. A C map is a fair baseline for the data
structure, and 1.9x is the measured cost of the managed-key API rather than of the hash
operation itself.

### Generic stack: monomorphization is free relative to C

`generic_stack` defines one `Stack<T>` type with `push`/`pop`/`len` methods and
instantiates it twice: for `i64` (1,000,000 pushes and pops) and for `String` (1,000
values). The C version writes the two typed stacks out by hand, since C has no generics.

lsaca takes 14.2 ms versus 17.4 ms for C (0.82x) and 16.1 ms for Rust, with all three
peaking at 12.6-12.7 MiB. 1.5 generics are monomorphized during type checking: each concrete
instantiation generates its own struct and functions in the emitted C, exactly like the
hand-written C versions, with no boxing and no dynamic dispatch. The row therefore shows
that the generic source form costs nothing at run time compared with the same code
written out for each type - the small gap is session noise.

### Function values in records

`fn_pipeline` stores a function value in a struct next to a non-copy `String` label:
`struct Rule { label: String, apply: fn(i64) -> i64 }`. It builds three rules, calls each
1,000 times, and calls a clone of the first rule 500 times. The C version uses a
`const char *` label and an `int64_t (*apply)(int64_t)` function pointer.

lsaca takes 9.9 ms versus 11.0 ms for C and 9.0 ms for Rust (0.91x/1.10x - all within
noise of each other), and peaks at 4.1 MiB versus C's 4.0 MiB. This is a workload that
was not expressible in 1.4 at all: storing a function value in a struct field produced
invalid C. Since 1.5 function values are ordinary aggregates, so the record can hold one
and the call goes through the stored handle; the cost is in the same range as a C
function pointer.

### Compile times: direct C back end, not CMake

A cold lsaca build of a small program is 0.74-1.05 s in this session (1.03-1.32 s in the
1.4 session, 1.15-1.72 s in the 1.3 session, 0.97-1.37 s in the 1.2.0 session). Two
things dominate it: compiling the C runtime translation units (about 100 KB of C in
total now that `lsaca_rt_map.c` was added) and compiling plus linking the generated
`main.c`; a single-file `gcc -O2` compile of the same program takes 0.19-0.30 s, so the
runtime and the project overhead account for most of the difference. A cached build is
13-20 ms because the content-addressed cache returns the previous executable without
invoking gcc at all.

In 1.1.0 the same cold build took 2.79-3.27 s because the pipeline included CMake
configuration and a Ninja build of the generated project. The 1.2 direct C compiler back
end (the default) skipped that layer, which made cold builds about 2.6-3.3x faster. The
old pipeline is still available with `--cmake` for comparison and for environments where
direct gcc invocation is not possible.

`compile_large` shows the scaling in generated-C size: 1,046,885 bytes (about 1.05 MB)
of C, 3.14 s cold, 227 ms cached - roughly 3-4x a small program's build time.

The answer to "lsaca compile speed" therefore depends on the cache: a first build is
slower than gcc by roughly 3-4x for small programs (mostly gcc on the runtime); rebuilding
an *unchanged* program is faster than either compiler, because the cache hit skips gcc
entirely.

### What 1.5 changed: generics, traits, iterators and HashMap

1.5 added generic structs/enums/functions/impls, user-defined traits with static
dispatch, lazy iterator adapters in the prelude, the builtin `HashMap<K, V>`, and the
ability to store function values in aggregates and compare aggregates with `==`. Four
new workloads measure those additions, each against the closest idiomatic C/Rust
equivalent:

| workload | lsaca (ms) | C (ms) | Rust (ms) | lsaca vs C | what it isolates |
| --- | ---: | ---: | ---: | ---: | --- |
| `iterator_pipeline` | 27.8 | 12.0 | 14.5 | 2.32x | lazy adapter chain versus an eager loop |
| `hashmap_wordcount` | 46.4 | 24.3 | 43.4 | 1.91x | builtin `HashMap` with owned `String` keys |
| `generic_stack` | 14.2 | 17.4 | 16.1 | 0.82x | two monomorphized instantiations |
| `fn_pipeline` | 9.9 | 11.0 | 9.0 | 0.91x | function value stored in a struct field |

The two ownership rows improved against the 1.4 session: the by-value tree traversal
moved from 677.3 ms / 101.1 MiB to 534.4 ms / 101.2 MiB, and the borrowed traversal from
35.8 ms / 19.9 MiB to 34.1 ms / 19.9 MiB - the borrowed traversal is now 15.7x faster and
uses 5.1x less peak memory than the by-value one. The rest of the suite is within the
session-to-session range already discussed; the new collection workloads are the only
rows that changed the shape of the table.

### Python and Node.js: start-up, interpretation and JIT

Python is 3.26-93.05x slower than C on these workloads. The widest gap is
`matrix_multiply` (1179.3 ms vs 12.7 ms, 93.05x), where 4.1 million multiply-adds run in
nested loops; next is `loop_sum` (805.6 vs 9.2 ms, 87.55x), where CPython executes 8
million integer additions in a bytecode loop; the narrowest is `fn_pipeline` (35.8 vs
11.0 ms, 3.26x), then `string_split` (4.70x) and `binary_trees_refs` (5.52x), where most of
the work happens in C-level methods or object references. Python is faster than lsaca
only on the by-value `binary_trees` row (97.4 vs 534.4 ms), for the aliasing reason
explained above.

Node.js lands between: 4.34-25.11x C. V8's JIT does well on the simple loops (`sort_i64`
142.7 ms, 4.78x; `mandelbrot` 119.8 ms, 4.34x; `sieve` 157.2 ms, 6.86x) but the numbers
include V8 start-up on every run (each measured run is a fresh process), and its memory
floor is visible in the memory table (40.2-89.3 MiB for the small workloads versus
4 MiB for native code).

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

   These are all reasonable code, but they do different amounts of low-level work, and
   that is exactly what the benchmarks are measuring.

2. **Contiguous values versus per-element objects.** C, Rust and lsaca store `Point`
   values contiguously in the vector/array; Python tuples and JS objects are individual
   heap objects referenced from the list/array. The memory rows for `vec_memory` are
   largely a consequence of that representation difference, not only of the runtime. The
   same inversion explains why Python (97.4 ms) beats lsaca on `binary_trees`
   (534.4 ms): reference semantics make aliasing cheap, while lsaca's by-value traversal
   copies subtrees. On the borrowed traversal the order reverses - lsaca 34.1 ms versus
   Python 83.4 ms - because not copying beats cheap aliasing.

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
   generics, user traits (static dispatch only), iterators, `HashMap`, and ownership with
   auto-derived `Copy`/`Clone`/`Drop`. It still has no trait objects (no `Box<dyn Trait>`),
   no library sort method, no unsigned or small integer types used in these programs, and
   no operator overloading beyond the builtin `==`. The workloads stay deliberately simple
   for that reason: `sort_i64` does not call a library sort in any language, and the
   string workloads use only the documented `String` API. `iterator_pipeline` compares the
   documented iterator surface against a hand-written loop rather than against an
   optimising iterator library. These numbers describe the 1.5.0 implementation, not the
   language's potential.

5. **What the lsaca release build does.** Release builds wrap `+ - *` (the generated C
   uses explicit unsigned casts), so there are no overflow checks on those operations.
   Division and remainder always go through the runtime's checked helpers because
   division by zero panics even in release; `string_build` performs one million
   `i % 1000` operations, `sort_i64` 200,000 LCG remainders and `matrix_multiply`'s fill
   loop uses `%` as well. Vector indexing is bounds-checked on every access, which is the
   main extra work in `matrix_multiply`'s inner loop (that row still comes out at parity
   in this session, so the checks are not a large cost at this size). The runtime's
   reference-count and copy-on-write checks are per string call. Moving a value is a
   pointer move with no allocation, a `.clone()` performs exactly the deep copy the source
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
   reported. This is a laptop with P/E cores, power management and background activity,
   and the suite ran while other builds were happening on the same machine. Treat
   single-digit percentage differences as noise, do not compare absolute numbers across
   pages or sessions, and prefer comparisons made inside one run. Session-level effects
   dominate absolute compile times: `rustc -O` took 0.93-1.65 s per file in the 1.3
   session versus 0.20-0.39 s here, while gcc stayed at 0.19-0.30 s. The medians absorb
   individual bad samples: the 1.3 run contained one Python `sieve` sample at 2.48 s
   against a 772.5 ms median, and the 1.2.0 run one Python `string_build` sample at
   5.38 s; median and minimum are unaffected. The 1.3 session also discarded a first
   attempt that was 1.2-1.6x slower across every language, C and Rust included - a
   uniform slowdown means a busy machine, not a slow compiler. This 1.5 run recorded no
   warnings: no sample needed re-measuring.

8. **Peak memory is working set, not allocation.** It includes the executable, runtime
   metadata and allocator arena. For Node.js the floor is 40-89 MiB and for Python
   ~11 MiB, so their entries measure the interpreter as much as the data. Sampling can
   miss very short spikes.

9. **Start-up is included in every runtime number.** Each measured run is a new process:
   dynamic loading, runtime initialisation and (for Python/Node.js) interpreter/VM start-up
   are all part of the time. For a ~11 ms workload like `loop_sum` in C, start-up is a
   noticeable fraction of the number.

10. **Compile-time columns answer different questions.** Cold lsaca includes C emission,
    compiling the five runtime translation units, compiling the generated C and linking;
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
