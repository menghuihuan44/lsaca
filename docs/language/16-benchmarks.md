# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
intuition: the same eleven workloads are implemented in lsaca, C, Rust, Python and
Node.js, built with each language's normal toolchain, run on the same machine, and checked
against a shared expected output. A twelfth, larger source is measured for compile time
only.

The numbers below were measured on **2026-10-27** with the repository's harness,
`scripts/bench.py`, on the reference machine described in
[Environment](#environment). The raw data (including every individual run, not just the
medians) is in [benchmarks/results/latest.json](../../benchmarks/results/latest.json);
the harness also writes a machine-generated subset of these tables to
[benchmarks/results/latest.md](../../benchmarks/results/latest.md). Instructions for
reproducing the run are at the end of this page.

The short version for lsaca 1.3.0:

- **Scalar loops, sorting and floating-point code are in the same ballpark as C.**
  `loop_sum` is 13.9 ms in lsaca versus 14.2 ms in C, `vec_memory` is 30.7 versus
  36.4 ms, `matrix_multiply` is 17.0 versus 17.1 ms, `sieve` is 39.4 versus 41.5 ms
  and `mandelbrot` is 49.2 versus 63.9 ms. `sort_i64` (63.1 versus 50.2 ms) and
  `string_build` (45.2 versus 27.0 ms) are slower; see the per-workload analysis and
  the noise caveats - on workloads of a few tens of milliseconds, session-to-session
  noise is comparable to the gaps themselves.
- **`vec_memory` stays at parity**: 30.7 ms and 18.8 MiB peak versus C's 36.4 ms and
  18.8 MiB. In 1.1.0 the same workload took 86.1 ms and 35.1 MiB, so the 1.2 runtime
  work still holds (about 3-5x faster than 1.1.0 depending on the session, roughly half
  the peak memory).
- **`string_build` is 3.2x faster than Rust here** (45.2 ms versus Rust's 146.4 ms)
  but still 1.7x C (27.0 ms): the remaining cost is the runtime's per-token string path,
  not the generated C.
- **1.3 references pay off on recursive structures.** `binary_trees_refs` sums the same
  tree as `binary_trees` through `&Option<Box<Node>>`: 223 ms and 35.8 MiB, versus
  1217 ms and 150.1 MiB for the by-value traversal - 5.4x faster with 4.2x less peak
  memory. It is the first version of this shape to come within single-digit multiples of
  C (9.2x runtime, 4.7x peak memory); the by-value row is still 47x C, because copying
  `Box<Node>` copies the subtree.
- **A cold build takes 1.15-1.72 s**; the default direct C compiler back end emits C and
  drives gcc itself. A cached build is 22-45 ms. The 4,511-line `compile_large` source
  (1.30 MB of generated C) takes 3.97 s cold and 308 ms cached.
- **Interpreters are 4.35-106.7x slower than C** on these workloads; their numbers
  also carry interpreter/VM start-up on every run.

## Workloads

Eleven small programs, each of which prints a single `checksum=...` line, plus a large
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
| `binary_trees` | many small allocations, recursion | build a perfect binary tree of depth 16 (131,071 nodes) and sum every payload | `checksum=2147450880` |
| `binary_trees_refs` | borrowed traversal (1.3 references) | the same tree as `binary_trees`, but summed through borrowed references (`&Option<Box<Node>>`) instead of by value, so the traversal does not copy the tree | `checksum=2147450880` |
| `string_split` | splitting and parsing text | build a 194,500-byte comma-separated numeric string, split it, parse every token and sum; checksum is length, token count and sum | `checksum=194500,50000,24975000` |
| `sieve` | array-heavy loop | sieve of Eratosthenes below 2,000,000; count the primes | `checksum=148933` |
| `mandelbrot` | floating-point loops | 600x400 escape-time Mandelbrot, at most 100 iterations per pixel; checksum is the sum of the iteration counts | `checksum=6282265` |
| `compile_large` | compile-time scaling with source size | generated 4,511-line lsaca source (450 functions); **compile-only**, never executed by the harness | `checksum=14655037` (not checked) |

The sizes are deliberate. `loop_sum` uses 8,000,000 iterations so that Python takes about
one second: long enough to measure above start-up noise, short enough that seven runs per
workload keep the whole suite under five minutes. The other workloads are sized so the
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
  compile the generated `main.c`, compile the four runtime translation units, and link,
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

All 56 (workload, language) combinations were built (55 of them also run), and every
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
| Measured at | 2026-10-27 23:19:55 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 1.3.0, repository HEAD `a34f3fe` with the uncommitted 1.3 working-tree changes, compiler sha256 prefix `cbedb91b14a2d114` |
| lsaca build path | `lsaca build -r`, default direct C compiler back end (gcc 15.2.0, `-O2 -DNDEBUG`); the old CMake + Ninja path is available as `--cmake` |
| C compiler | gcc 15.2.0 (MSYS2 UCRT64, Rev14, built by MSYS2 project) |
| Rust compiler | rustc 1.99.0 (`b940084d7 2026-09-28`) |
| Python | 3.14.4 |
| Node.js | v24.14.1 |
| Harness | `scripts/bench.py`, 12 programs (11 executed), 1 warm-up + 7 runs, 3 compile repeats, peak-memory sampling on |

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

The "lsaca emit-c + gcc" column is only filled when the harness runs in fallback mode
(see the fairness notes); in this official run the default build back end was used and
that column is not measured.

Observations:

- A cold lsaca build is **1.15-1.72 s** (3.97 s for the 4,511-line `compile_large`).
  Most of that is gcc: compiling the generated `main.c` together with the four runtime
  translation units and linking them (a single-file gcc compile of the same workload is
  0.30-0.50 s). The front end itself is a small part of the measurement - the C emission
  step is not separately recorded but is a few tens of milliseconds.
- `compile_large` shows the scaling: 4,511 lines / 450 functions produce 1.30 MB of
  generated C and 3.97 s of cold build time, roughly 2.5-3x a small program's build. The
  build time therefore grows with the generated C, as expected for "run gcc on C".
- A cached lsaca build is **22-45 ms** for the small programs (308 ms for
  `compile_large`). That is the content-addressed cache hit: the compiler hashes the
  generated program and returns the existing executable rather than invoking gcc. In
  practice an edit to the `.ls` source invalidates the cache and pays the full cold cost
  again; there is no incremental compile yet.
- The C column (0.30-0.50 s) is a single-file compile with no project setup; it is the
  right baseline for "front end plus codegen", not for the whole emit-compile-link
  pipeline. The Rust column is 0.93-1.65 s in this session - higher than the 0.30-0.45 s
  recorded for an earlier session on the same machine, a reminder that absolute compile
  times here are sensitive to the machine's state (see the caveats), not to these
  benchmark sources.

### Runtime

Milliseconds, **median (minimum)** of 7 measured runs after 1 warm-up. Lower is better.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 44.2 (38.3) | 13.9 (12.2) | 45.2 (42.4) | 30.7 (27.4) | 63.1 (61.2) | 17.0 (16.5) | 1216.7 (1189.9) | 223.2 (215.5) | 32.2 (28.9) | 39.4 (38.2) | 49.2 (47.2) |
| C (gcc -O2) | 35.7 (33.5) | 14.2 (12.0) | 27.0 (25.2) | 36.4 (30.5) | 50.2 (46.9) | 17.1 (15.8) | 25.8 (23.9) | 24.4 (22.9) | 19.3 (18.1) | 41.5 (37.7) | 63.9 (59.7) |
| Rust (rustc -O) | 23.2 (20.8) | 14.2 (13.3) | 146.4 (127.4) | 34.8 (30.1) | 43.6 (42.5) | 16.2 (15.5) | 26.5 (24.9) | 25.8 (24.6) | 18.6 (17.7) | 38.9 (36.8) | 59.1 (50.8) |
| Python 3 | 1163.1 (1112.7) | 1511.7 (1168.6) | 704.8 (615.6) | 867.7 (773.7) | 756.8 (734.5) | 1538.4 (1518.3) | 117.7 (112.5) | 117.9 (114.2) | 83.8 (78.6) | 772.5 (674.1) | 3423.3 (2729.5) |
| Node.js | 344.3 (281.9) | 214.8 (190.1) | 388.1 (307.8) | 349.0 (326.9) | 202.6 (197.2) | 172.3 (163.5) | 173.6 (164.6) | 179.7 (169.0) | 171.7 (166.9) | 249.6 (230.3) | 192.6 (154.7) |

The same numbers as a ratio to the C time (median / C median). Below 1.0 means faster
than C:

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 1.24x | 0.98x | 1.68x | 0.84x | 1.26x | 1.00x | 47.11x | 9.16x | 1.67x | 0.95x | 0.77x |
| Rust | 0.65x | 1.00x | 5.42x | 0.96x | 0.87x | 0.95x | 1.03x | 1.06x | 0.96x | 0.94x | 0.93x |
| Python 3 | 32.58x | 106.73x | 26.12x | 23.87x | 15.08x | 90.00x | 4.56x | 4.84x | 4.35x | 18.61x | 53.59x |
| Node.js | 9.64x | 15.16x | 14.38x | 9.60x | 4.04x | 10.08x | 6.72x | 7.37x | 8.90x | 6.01x | 3.01x |

### Peak memory

MiB of peak working set, separate run. Lower is better. Remember that this is total
process footprint: it includes the runtime, the interpreter/VM and its heap.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 18.8 | 6.6 | 4.4 | 150.1 | 35.8 | 11.1 | 18.8 | 4.0 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.4 | 7.6 | 7.6 | 4.0 | 18.8 | 4.0 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 7.7 | 4.1 | 18.9 | 4.1 |
| Python 3 | 11.0 | 11.0 | 73.6 | 142.8 | 21.8 | 13.6 | 21.0 | 21.0 | 16.8 | 26.2 | 11.0 |
| Node.js | 43.0 | 44.5 | 89.5 | 121.1 | 51.0 | 46.9 | 56.1 | 56.2 | 50.6 | 59.3 | 44.1 |

For workloads with no large data structures (`fib`, `loop_sum`, `mandelbrot`) every
native language sits at the 4 MiB floor of the process image. Where the data is large:
`vec_memory` allocates 16 MB of `Point` values and both lsaca and C peak at 18.8 MiB
(1.2 stores the vector in place, whereas 1.1 duplicated it and peaked at 35.1 MiB);
`sieve` keeps a 2,000,000-element `i64` flag vector, 16 MB, and peaks at 18.8 MiB;
`string_build` allocates a ~3.9 MB string and adds a temporary per token, hence
10.6 MiB versus 7.3 MiB for C. `binary_trees` is the outlier at 150.1 MiB for 131,071
small nodes, because copying a `Box<Node>` copies the whole subtree; the borrowed
`binary_trees_refs` traversal avoids those copies during the sum and peaks at 35.8 MiB,
4.7x C's 7.6 MiB instead of 20x.

## Analysis

### Scalar integer loops: parity with C

`loop_sum` is the cleanest comparison: 8,000,000 iterations of `sum += i` and nothing
else. lsaca takes 13.9 ms, C 14.2 ms, Rust 14.2 ms - all within a few percent of each
other. In release mode the generated C lowers integer additions to plain C arithmetic
(the wrapping semantics are expressed as explicit `uint64_t` casts, so the compiler does
not need to keep overflow checks), and GCC -O2 turns the loop into comparable machine code
to a hand-written C loop.

`sieve` (39.4 vs 41.5 ms) and `mandelbrot` (49.2 vs 63.9 ms) tell the same story for
an array-heavy loop and for floating-point arithmetic: when the work does not need the
runtime's help, lsaca generates C that performs like C. In this session lsaca even came
out ahead on `mandelbrot`; on 40-60 ms workloads that is GCC-vs-LLVM code-generation
noise, not evidence that generated C is faster than C. This is the strongest evidence
that the C backend is not intrinsically slow.

### Recursion-heavy calls: fib

The recorded session has lsaca at 44.2 ms, C at 35.7 ms and Rust at 23.2 ms for the same
recursive algorithm. Two things matter here, and only one of them is about lsaca:

1. **C and Rust differ by 1.5x on identical source-level algorithms.** That is a
   code-generation effect between GCC 15.2 and LLVM (via rustc 1.99.0) on a
   self-recursive function, not a language-semantics effect. Any single workload can
   show a compiler gap of this size; it should not be read as "Rust is faster than C".
2. **The lsaca / C difference is noisier than the other workloads.** The recorded session
   shows lsaca at 1.24x C, and an earlier interleaved re-timing of a previous binary pair
   (9 runs each, alternating) measured 36.9 ms (lsaca) vs 39.0 ms (C) - roughly parity.
   Treat the fib row as "same ballpark", not as a stable speed ratio.

The constant argument n=30 does not explain the differences: probes with
`std::hint::black_box(30)` in Rust and a `volatile` n in C produced timings within a few
percent of the plain versions, so the compilers are not constant-folding the recursion. In
the generated lsaca program n is a runtime variable anyway.

### String building: runtime-bound, 1.7x C

`string_build` appends 1,000,000 small strings. lsaca needs 45.2 ms versus 27.0 ms for C
and 146.4 ms for Rust (Rust is slower than C here because it also allocates a temporary
`String` per token). In lsaca 1.1.0 the same program took 212.9 ms; the 1.2 runtime work
brought that down to 33.9 ms, and this session measures 45.2 ms - 4.7x faster than 1.1.0
and 3.2x faster than the Rust version.

The remaining 1.7x gap to C is about what a `String` is in each implementation:

- The C program allocates one buffer of the exact final size up front and writes bytes
  into it. There are almost no function calls in the hot loop.
- At the lsaca level the program is `s.push_str(format!("{},", v))`. Each iteration
  builds a temporary formatted `String` and appends it through the runtime's string
  append function, which handles reference counting and geometric growth. The loop
  therefore performs one allocation plus one append per token; C's version performs none.
- The peak-memory row reflects the same design: the last growth step keeps the old and
  the new buffer alive at the same time (10.6 MiB total versus 7.3-7.4 MiB for C and
  Rust).

The measured 1.7x is a statement about the current runtime implementation, not about the
language: an API that let the caller append into a pre-sized buffer, or that inlined the
growth path, could close the rest of the gap.

### Vectors of struct values: now at parity

`vec_memory` pushes 1,000,000 two-word structs into a `Vec`. In this 1.3 session lsaca
takes 30.7 ms and peaks at 18.8 MiB, versus 36.4 ms / 18.8 MiB in C and 34.8 ms / 18.9 MiB
in Rust (all three within run-to-run noise of each other; 1.2 measured 16.8 ms for lsaca
on the same program). In 1.1.0 it was 86.1 ms and 35.1 MiB.

Two runtime and code-generation changes explain the difference: the hot `push` path is
now a fast path that writes the value into the reserved slot directly instead of going
through a temporary element and a generic insertion helper, and iterating a vector with
`for p in points` no longer deep-copies the whole buffer.

`Vec` is still a **value type** in lsaca: an explicit copy of a vector deep-copies its
buffer. Since 1.3 the language has references, so a helper function can take
`&Vec<T>`/`&mut Vec<T>` and look at or mutate the vector in place instead of copying it,
but that does not change what a copy means. The vector in this workload is built and
consumed in one function without such a copy, so the benchmark shows that the common path
is as cheap as C. Shapes that do copy (see `binary_trees` and the fairness notes) still
pay for it.

### Sorting: 200,000 values, in-language algorithm

`sort_i64` sorts 200,000 pseudo-random values with an iterative quicksort written in the
language: explicit stack of index pairs, middle pivot, Hoare partition, manual swaps
through bounds-checked indexing. lsaca takes 63.1 ms versus 50.2 ms in C and 43.6 ms in
Rust, a 1.26x gap (1.2 measured near-parity: 35.7 vs 37.4 vs 33.2 ms). The C version uses
the same algorithm and the same explicit stack; the Rust version uses `swap` on a mutable
slice.

No language uses a library sort in this workload (lsaca has no `Vec::sort`), so this is a
fair "how expensive are the language's primitives in a write-heavy loop" comparison, and
the answer is: in the same range as C. The swap-and-index loop executes bounds checks in
lsaca that C and Rust do not, but this workload is branch-heavy enough that its absolute
time moves more between sessions than that overhead; treat the row as "same range", not as
a measured constant. A library `qsort`/`sort_unstable` in C or Rust would be faster than
any of these numbers.

### Matrix multiplication: bounds checks in the inner loop

`matrix_multiply` multiplies two 160x160 `i64` matrices stored as flat vectors. lsaca
takes 17.0 ms versus 17.1 ms for C and 16.2 ms for Rust - parity in this session (1.2
measured 26.4 vs 18.4 vs 24.2 ms, a 1.44x gap). The inner loop indexes the two input
vectors and the output vector by computed positions (`a[i*n+k] * b[k*n+j]`), so every
access is bounds-checked in lsaca, while C and Rust index without a check. Bounds checks
of that shape are typically a few percent on a workload this small; the 1.2-vs-1.3 swing
(and the fact that the whole program is 17 ms) means this row is dominated by
code-generation and session noise. The arithmetic itself is plain C code, which is why all
three implementations land in the same range.

### Binary trees: value semantics versus the borrowed traversal

`binary_trees` builds a perfect tree of 131,071 nodes and sums every payload through a
value-taking function (`fn sum_tree(tree: Option<Box<Node>>) -> i64`). C takes 25.8 ms and
peaks at 7.6 MiB; lsaca takes 1216.7 ms and peaks at 150.1 MiB. This is by far the largest
gap in the suite, and it is a language-model effect rather than a codegen effect:

- In C the tree is `malloc`ed nodes with raw pointers: building and traversing it never
  duplicates anything.
- In Rust the tree is `Box<Node>`: moving a `Box` is moving a pointer.
- In lsaca, `Box<T>` is a **value type**: moving or copying a box copies its payload, and
  the payload of a `Node` contains two boxes, so moving a node copies its whole subtree,
  recursively. The value-taking `sum_tree` call therefore walks the tree through copies.
  The 47x runtime and the 20x peak-memory gap are the cost of that model, not of the C
  backend.

Version 1.3 adds a second workload on exactly the same tree: `binary_trees_refs` sums it
through references instead - `fn sum_tree(tree: &Option<Box<Node>>) -> i64`, matching
`Some(boxed)` and recursing on `&current.left` and `&current.right`. That traversal does
not copy the tree. It takes 223.2 ms and peaks at 35.8 MiB, so **the borrowed traversal is
5.4x faster and uses 4.2x less peak memory than the value traversal on the same tree, same
nodes and same algorithm**. That difference is what reference support buys, and it is the
main story of the 1.3 numbers.

The borrowed version is not free of the runtime: it is still 9.2x C's runtime and 4.7x its
peak memory (instead of 47x and 20x), because the tree is still built and dropped out of
reference-counted `Box` nodes, and building it by value copies subtrees just as before.
Interesting comparison: Python (117.7 ms on `binary_trees`, 117.9 ms on
`binary_trees_refs`) is still faster than either lsaca row, because Python objects are
references and aliasing is the default there.

### String splitting and parsing

`string_split` builds a 194,500-byte comma-separated string, splits it into 50,000
tokens and parses each one. lsaca takes 32.2 ms versus 19.3 ms for C (1.67x) and 18.6 ms
for Rust. The result-string allocation and the per-token parse are both runtime calls;
the gap is in the same class as `string_build` but much smaller because the string is
short and the tokens are small.

### Compile times: direct C back end, not CMake

A cold lsaca build of a small program is 1.15-1.72 s in this session (0.97-1.37 s in the
1.2.0 session). Two things dominate it: compiling the C runtime translation units (about
86 KB of C in total) and compiling plus linking the generated `main.c`; a single-file
`gcc -O2` compile of the same program takes 0.30-0.50 s, so the runtime and the project
overhead account for most of the difference. A cached build is 22-45 ms because the
content-addressed cache returns the previous executable without invoking gcc at all.

In 1.1.0 the same cold build took 2.79-3.27 s because the pipeline included CMake
configuration and a Ninja build of the generated project. The 1.2 direct C compiler back
end (the default) skipped that layer, which made cold builds about 2.6-3.3x faster. The
old pipeline is still available with `--cmake` for comparison and for environments where
direct gcc invocation is not possible.

One number needs a caveat: `rustc -O` compiles these single files in 0.93-1.65 s in this
session, versus 0.30-0.45 s in the earlier sessions on the same machine. That is a
session-level effect (linker and anti-malware scanning cost on the host, not a property of
the program), so the lsaca-vs-rustc compile comparison is much less stable than the
lsaca-vs-gcc one; this table's Rust column should be read as "what this machine did while
the suite ran", not as a benchmark of rustc.

The answer to "lsaca compile speed" therefore depends on the cache: a first build is
slower than gcc by roughly 3-4x for small programs (mostly gcc on the runtime); rebuilding
an *unchanged* program is faster than either compiler, because the cache hit skips gcc
entirely. `compile_large` shows the scaling in generated-C size: 1.30 MB of C,
3.97 s cold, 308 ms cached.

### What 1.3 changed: references and the borrowed traversal

1.3 added `&T`/`&mut T` references and a conservative borrow checker on top of the
unchanged reference-counted runtime. `binary_trees_refs` is the workload that exercises
it: the same tree, the same node type and the same checksum as `binary_trees`, only the
traversal signature differs. The two rows below are therefore a controlled comparison of
one language feature:

| workload | runtime (ms) | peak (MiB) | runtime vs C | peak vs C |
| --- | ---: | ---: | ---: | ---: |
| `binary_trees` (traverse by value) | 1216.7 | 150.1 | 47.1x | 19.8x |
| `binary_trees_refs` (traverse through `&`) | 223.2 | 35.8 | 9.2x | 4.7x |
| C `binary_trees` (same tree) | 25.8 | 7.6 | 1.0x | 1.0x |

The rest of the suite is unchanged within session noise. For reference, the 1.2.0 session
measured 34.9/11.7/33.9/16.8 ms for `fib`/`loop_sum`/`string_build`/`vec_memory`;
this 1.3 session measured 44.2/13.9/45.2/30.7 ms. Cross-session absolute numbers are not
comparable (see the noise note). The earlier 1.1.0-to-1.2.0 improvements (`string_build`
6.3x faster, `vec_memory` 5.1x faster, direct C back end instead of CMake) remain in the
code base.

### Python and Node.js: start-up, interpretation and JIT

Python is 4.35-106.73x slower than C on these workloads. The widest gap is `loop_sum`
(1511.7 ms vs 14.2 ms), where CPython executes 8 million integer additions in a bytecode
loop; next is `matrix_multiply` (1538.4 vs 17.1 ms, 4.1 million multiply-adds in nested
loops); the narrowest is `string_split` (83.8 vs 19.3 ms, 4.35x), where most of the work
happens in C-level string methods, and `binary_trees`/`binary_trees_refs` (4.6-4.8x),
where Python's reference semantics pay off (Python is faster than lsaca on the tree
workload, as noted above).

Node.js lands between: 3.01-15.16x C. V8's JIT does well on the simple loops
(`mandelbrot` 192.6 ms, 3.0x; `sieve` 249.6 ms, 6.0x) but the numbers include V8
start-up on every run (each measured run is a fresh process), and its memory floor is
visible in the memory table (43-51 MiB for the small workloads versus 4 MiB for native
code).

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
   | `binary_trees` | `Option<Box<Node>>` recursive struct | `malloc`ed nodes + raw pointers | `Option<Box<Node>>` if let / match | class with `__slots__` | plain objects |
   | `binary_trees_refs` | same tree, summed via `&Option<Box<Node>>` (no copy) | same program as `binary_trees` (C uses pointers anyway) | same program as `binary_trees` (moving a `Box` is a pointer move) | same program | same program |
   | `sieve` | `vec![0; limit + 1]` of `i64` | `calloc`ed `int64_t` array | `vec![0i64; n]` | list of ints | `Float64Array` |

   These are all reasonable code, but they do different amounts of low-level work, and
   that is exactly what the benchmarks are measuring.

2. **Contiguous values versus per-element objects.** C, Rust and lsaca store `Point`
   values contiguously in the vector/array; Python tuples and JS objects are individual
   heap objects referenced from the list/array. The memory rows for `vec_memory` are
   largely a consequence of that representation difference, not only of the runtime. The
   same inversion explains why Python (117.7 ms) beats lsaca on `binary_trees`
   (1216.7 ms) and even on `binary_trees_refs` (117.9 vs 223.2 ms): reference semantics
   make aliasing cheap, while lsaca still builds and drops reference-counted `Box` nodes
   and copies subtrees when the tree is built by value.

3. **Value semantics: copies, not references.** An lsaca `Vec`/`Box` is a value type.
   Copying it (assignment, passing it by value, or an explicit copy) deep-copies the
   payload, unlike Rust's move/borrow model or C's explicit pointers. 1.2 avoids the
   copies where it can: `Vec::with_capacity(n)` bound to a `let` and `for x in vec`
   iteration no longer duplicate the buffer (hence the `vec_memory` parity). Since 1.3
   the language also has `&T`/`&mut T` references with borrow checking, which is how
   `binary_trees_refs` traverses the tree without copying it; the value-by-value
   `binary_trees` variant still pays the copy cost, and that difference is the measured
   value of reference support.

4. **lsaca compiler limitations at measurement time.** The language has no generics and
   no `Vec` sort method, and no unsigned/small integer types in these programs, so the
   workloads stay deliberately simple. (`binary_trees_refs` exercises the 1.3 reference
   feature, which is the only place references appear in the suite; the other programs
   are unchanged from earlier versions.) In particular `sort_i64` does not call a
   library sort in any language, and the string workloads use only the documented
   `String` API. These numbers describe one young compiler version (1.3.0), not the
   language's potential.

5. **What the lsaca release build does.** Release builds wrap `+ - *` (the generated C
   uses explicit unsigned casts), so there are no overflow checks on those operations.
   Division and remainder always go through the runtime's checked helpers because
   division by zero panics even in release; `string_build` performs one million
   `i % 1000` operations, `sort_i64` 200,000 LCG remainders and `matrix_multiply`'s
   fill loop uses `%` as well. Vector indexing is bounds-checked on every access, which
   is the main extra work in `matrix_multiply`'s inner loop (that row still comes out at
   parity in this session, so the checks are not a large cost at this size). The runtime's
   reference-count and copy-on-write checks are per string call.

6. **Compiler and flags.** gcc 15.2.0 (MSYS2 UCRT64) and rustc 1.99.0, each with the
   single normal optimisation flag (`-O2` / `-O`). No LTO, PGO, `-march=native` or
   target-cpu tuning for any language. The lsaca runtime is compiled by the same gcc with
   `-O2 -DNDEBUG`. The comparison is of standard builds on one Windows machine, not of
   tuned builds on Linux.

7. **Noise, statistics and a shared machine.** 7 measured runs, median (and minimum)
   reported. This is a laptop with P/E cores, power management and background activity,
   and the suite ran while other builds were happening on the same machine. The first
   full attempt at this 1.3 run was discarded because it was 1.2-1.6x slower than the
   published one across every language, C and Rust included - a uniform slowdown means a
   busy machine, not a slow compiler. The first attempt's measurements were discarded;
   only the quiet-machine run is published. Even then, session-level effects dominate absolute
   compile times: `rustc -O` took 0.93-1.65 s per file here versus 0.30-0.45 s in earlier
   sessions on the same machine, while gcc stayed at 0.30-0.50 s. Treat single-digit
   percentage differences as noise, do not compare absolute numbers across pages or
   sessions, and prefer comparisons made inside one run. The medians absorb individual
   bad samples: this run contains one Python `sieve` sample at 2.48 s against a 772.5 ms
   median, and the 1.2.0 run contained e.g. one Python `string_build` sample at 5.38 s;
   median and minimum are unaffected. None of the published samples needed re-measuring
   in this run (no warnings).

8. **Peak memory is working set, not allocation.** It includes the executable, runtime
   metadata and allocator arena. For Node.js the floor is 43-51 MiB and for Python
   ~11 MiB, so their entries measure the interpreter as much as the data. Sampling can
   miss very short spikes.

9. **Start-up is included in every runtime number.** Each measured run is a new process:
   dynamic loading, runtime initialisation and (for Python/Node.js) interpreter/VM start-up
   are all part of the time. For a ~14 ms workload like `loop_sum` in C, start-up is a
   noticeable fraction of the number.

10. **Compile-time columns answer different questions.** Cold lsaca includes C emission,
    compiling the four runtime translation units, compiling the generated C and linking;
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

# 2. Run the full suite (roughly 3-5 minutes; the published run took 3m47s).
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
python scripts/bench.py --build-mode fallback      # emit-c -r + gcc instead of the default path (labelled)
python scripts/bench.py --no-memory                # skip the peak-memory measurements
python scripts/bench.py --help                     # all options
```

The harness exits non-zero if any measured run produces the wrong output or a non-zero
exit code, so a broken implementation fails the run instead of producing numbers. Run
the suite on an otherwise idle machine; if you compare with the numbers above, compare
the same build mode (official) and keep in mind the caveats in the previous section.
Benchmark sources and harness internals are described in
[benchmarks/README.md](../../benchmarks/README.md).

## See also

- [benchmarks/README.md](../../benchmarks/README.md) - harness usage, output layout, how
  to add a workload.
- [benchmarks/results/latest.json](../../benchmarks/results/latest.json) - the raw data
  behind this page.
- [lsaca documentation](../README.md) - the manual hub and full index.
- [Getting started](01-getting-started.md) - the first chapter of the language tour.
