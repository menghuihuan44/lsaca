# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
intuition: the same eleven workloads are implemented in lsaca, C, Rust, Python and
Node.js, built with each language's normal toolchain, run on the same machine, and checked
against a shared expected output. A twelfth, larger source is measured for compile time
only.

The numbers below were measured on **2026-10-28** with the repository's harness,
`scripts/bench.py`, on the reference machine described in
[Environment](#environment). The raw data (including every individual run, not just the
medians) is in [benchmarks/results/latest.json](../../benchmarks/results/latest.json);
the harness also writes a machine-generated subset of these tables to
[benchmarks/results/latest.md](../../benchmarks/results/latest.md). Instructions for
reproducing the run are at the end of this page.

The short version for lsaca 1.4.0:

- **Scalar loops, sorting and floating-point code are in the same ballpark as C.**
  `loop_sum` is 12.7 ms in lsaca versus 12.3 ms in C, `vec_memory` 23.4 versus
  22.4 ms, `matrix_multiply` 16.1 versus 16.6 ms, `sieve` 36.6 versus 34.5 ms and
  `mandelbrot` 34.3 versus 34.9 ms. This session `fib` came out at 35.8 versus 43.8 ms
  and `sort_i64` at 41.4 versus 37.2 ms - all sub-100 ms workloads, where
  session-to-session noise is comparable to the gaps themselves (see the noise caveats
  and the per-workload analysis).
- **`vec_memory` stays at parity**: 23.4 ms and 18.8 MiB peak versus C's 22.4 ms and
  18.8 MiB. In 1.1.0 the same workload took 86.1 ms and 35.1 MiB, so the runtime and
  code-generation work since then still holds (about 3-4x faster than 1.1.0, roughly
  half the peak memory).
- **`string_build` is 2.4x faster than Rust here** (39.4 ms versus Rust's 96.1 ms)
  but still 2.4x slower than C (16.7 ms): the remaining cost is the runtime's per-token
  string path, not the generated C.
- **1.4 ownership makes the cost of copying explicit.** `binary_trees_refs` sums the
  same tree through `&Option<Box<Node>>` and copies nothing: 35.8 ms and 19.9 MiB,
  which is 1.33x C's runtime and 2.6x its peak memory. The value-taking
  `binary_trees` variant has to `clone()` each child subtree explicitly, because
  moving a field out of a value is `E0507`; it pays 677.3 ms and 101.1 MiB - 18.9x
  slower and 5.1x more peak memory than the borrowed traversal. That gap is the measured
  cost of value semantics used naively, and 1.4 makes it visible in the source instead
  of hiding it in implicit copies.
- **A cold build takes 1.03-1.32 s** for the small programs; the default direct C
  compiler back end emits C and drives gcc itself. A cached build is 20-36 ms. The
  4,511-line `compile_large` source (1.05 MB of generated C) takes 4.03 s cold and
  324 ms cached.
- **Interpreters are 4.1-101.9x slower than C** on these workloads; their numbers
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
| `binary_trees` | many small allocations, recursion | build a perfect binary tree of depth 16 (131,071 nodes) and sum every payload through a by-value function that `.clone()`s each child subtree; the deep copies are part of the measured cost | `checksum=2147450880` |
| `binary_trees_refs` | borrowed traversal (1.4 references) | the same tree as `binary_trees`, but summed through borrowed references (`&Option<Box<Node>>`) instead of by value, so the traversal does not copy the tree | `checksum=2147450880` |
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
| Measured at | 2026-10-28 22:50:42 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 1.4.0, repository HEAD `4074eb0` with the uncommitted 1.4 working-tree changes, compiler sha256 prefix `f3eabc91d7f0806b` |
| lsaca build path | `lsaca build -r`, default direct C compiler back end (gcc 15.2.0, `-O2 -DNDEBUG`); the old CMake + Ninja path is available as `--cmake` |
| C compiler | gcc 15.2.0 (MSYS2 UCRT64, Rev14, Built by MSYS2 project) |
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

The "lsaca emit-c + gcc" column is only filled when the harness runs in fallback mode
(see the fairness notes); in this official run the default build back end was used and
that column is not measured.

Observations:

- A cold lsaca build is **1.03-1.32 s** for the eleven runtime workloads. One sample,
  `matrix_multiply` at 2.82 s, is an outlier against the rest; its generated C is
  15,718 bytes, in the middle of the pack, so it is not a source-size effect - treat it
  as a session artifact. Most of the cold cost is gcc: compiling the generated
  `main.c` together with the four runtime translation units and linking them (a
  single-file gcc compile of the same workload is 0.27-0.42 s). The front end itself is
  a small part of the measurement - the C emission step is not separately recorded but
  is a few tens of milliseconds.
- `compile_large` shows the scaling: 4,511 lines / 450 functions produce 1,046,885
  bytes (about 1.05 MB) of generated C and 4.03 s of cold build time, roughly 3x a
  small program's build. The build time therefore grows with the generated C, as
  expected for "run gcc on C".
- A cached lsaca build is **20-36 ms** for the small programs (324 ms for
  `compile_large`). That is the content-addressed cache hit: the compiler hashes the
  generated program and returns the existing executable rather than invoking gcc. In
  practice an edit to the `.ls` source invalidates the cache and pays the full cold cost
  again; there is no incremental compile yet.
- The C column (0.27-0.42 s) is a single-file compile with no project setup; it is the
  right baseline for "front end plus codegen", not for the whole emit-compile-link
  pipeline. In this session `rustc -O` took 0.31-0.42 s, comparable to gcc; earlier
  sessions on the same machine measured 0.93-1.65 s for the same rustc invocations (see
  the caveats), which is a reminder that absolute compile times here are sensitive to
  the machine's state, not to these benchmark sources.

### Runtime

Milliseconds, **median (minimum)** of 7 measured runs after 1 warm-up. Lower is better.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 35.8 (33.9) | 12.7 (12.2) | 39.4 (38.2) | 23.4 (19.2) | 41.4 (39.1) | 16.1 (15.8) | 677.3 (662.8) | 35.8 (35.6) | 31.7 (22.5) | 36.6 (33.1) | 34.3 (33.6) |
| C (gcc -O2) | 43.8 (34.6) | 12.3 (12.0) | 16.7 (15.5) | 22.4 (19.1) | 37.2 (33.4) | 16.6 (15.6) | 25.1 (23.6) | 26.9 (24.9) | 19.5 (18.5) | 34.5 (34.1) | 34.9 (33.2) |
| Rust (rustc -O) | 24.9 (21.1) | 19.0 (14.3) | 96.1 (89.5) | 23.9 (21.6) | 35.6 (35.0) | 15.6 (15.0) | 24.6 (23.9) | 26.3 (25.2) | 17.2 (16.5) | 36.2 (34.9) | 38.3 (34.9) |
| Python 3 | 1180.9 (1116.6) | 1147.1 (1072.2) | 471.4 (466.0) | 636.0 (573.2) | 803.3 (729.1) | 1689.4 (1535.4) | 111.9 (110.9) | 146.2 (124.2) | 80.3 (77.6) | 599.0 (581.5) | 2869.9 (2773.7) |
| Node.js | 346.6 (329.6) | 165.6 (154.9) | 236.6 (225.7) | 261.1 (255.8) | 201.1 (189.3) | 161.7 (153.4) | 154.9 (152.4) | 184.1 (175.8) | 169.5 (159.2) | 201.8 (177.9) | 162.2 (156.1) |

The same numbers as a ratio to the C time (median / C median). Below 1.0 means faster
than C:

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 0.82x | 1.04x | 2.36x | 1.04x | 1.11x | 0.97x | 27.01x | 1.33x | 1.62x | 1.06x | 0.98x |
| Rust | 0.57x | 1.55x | 5.75x | 1.07x | 0.96x | 0.94x | 0.98x | 0.98x | 0.88x | 1.05x | 1.10x |
| Python 3 | 26.97x | 93.59x | 28.21x | 28.34x | 21.58x | 101.92x | 4.46x | 5.44x | 4.11x | 17.37x | 82.25x |
| Node.js | 7.92x | 13.51x | 14.16x | 11.63x | 5.40x | 9.76x | 6.18x | 6.85x | 8.67x | 5.85x | 4.65x |

### Peak memory

MiB of peak working set, separate run. Lower is better. Remember that this is total
process footprint: it includes the runtime, the interpreter/VM and its heap.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | binary_trees_refs | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.1 | 10.4 | 18.8 | 6.6 | 4.4 | 101.1 | 19.9 | 11.1 | 18.9 | 4.0 |
| C (gcc -O2) | 4.0 | 4.0 | 7.4 | 18.8 | 5.1 | 4.4 | 7.7 | 7.7 | 4.0 | 18.9 | 4.1 |
| Rust (rustc -O) | 4.2 | 4.2 | 7.4 | 18.9 | 5.2 | 4.3 | 7.7 | 7.7 | 4.4 | 18.9 | 4.2 |
| Python 3 | 11.1 | 11.0 | 73.8 | 142.2 | 21.4 | 13.7 | 21.1 | 21.1 | 17.0 | 26.3 | 11.1 |
| Node.js | 42.8 | 45.7 | 89.6 | 121.1 | 50.9 | 46.9 | 56.2 | 56.2 | 50.6 | 60.0 | 44.9 |

For workloads with no large data structures (`fib`, `loop_sum`, `mandelbrot`) every
native language sits at the 4 MiB floor of the process image. Where the data is large:
`vec_memory` allocates 16 MB of `Point` values and both lsaca and C peak at 18.8 MiB;
`sieve` keeps a 2,000,000-element `i64` flag vector, 16 MB, and peaks at 18.9 MiB;
`string_build` allocates a ~3.9 MB string and adds a temporary per token, hence
10.4 MiB versus 7.4 MiB for C. `string_split` peaks at 11.1 MiB versus 4.0 MiB for C
because the split tokens and the parsed values are heap `String`s on the lsaca side.
`binary_trees` is the outlier at 101.1 MiB for 131,071 small nodes, because the by-value
traversal clones whole subtrees; the borrowed `binary_trees_refs` traversal copies
nothing and peaks at 19.9 MiB, 2.6x C's 7.7 MiB instead of 13.2x.

## Analysis

### Scalar integer loops: parity with C

`loop_sum` is the cleanest comparison: 8,000,000 iterations of `sum += i` and nothing
else. lsaca takes 12.7 ms and C 12.3 ms - within a few percent - while Rust measured
19.0 ms on the same program this session (a reminder that a compiler can lose a simple
loop by 1.5x). In release mode the generated C lowers integer additions to plain C
arithmetic (the wrapping semantics are expressed as explicit `uint64_t` casts, so the
compiler does not need to keep overflow checks), and GCC -O2 turns the loop into
comparable machine code to a hand-written C loop.

`sieve` (36.6 vs 34.5 ms) and `mandelbrot` (34.3 vs 34.9 ms) tell the same story for
an array-heavy loop and for floating-point arithmetic: when the work does not need the
runtime's help, lsaca generates C that performs like C. In this session lsaca even came
out slightly ahead on `mandelbrot`; on 30-40 ms workloads that is GCC-vs-LLVM
code-generation noise, not evidence that generated C is faster than C. This is the
strongest evidence that the C backend is not intrinsically slow.

### Recursion-heavy calls: fib

The recorded session has lsaca at 35.8 ms, C at 43.8 ms and Rust at 24.9 ms for the same
recursive algorithm. Two things matter here, and only one of them is about lsaca:

1. **C and Rust differ by 1.76x on identical source-level algorithms.** That is a
   code-generation effect between GCC 15.2 and LLVM (via rustc 1.99.0) on a
   self-recursive function, not a language-semantics effect. Any single workload can
   show a compiler gap of this size; it should not be read as "Rust is faster than C".
2. **The lsaca / C difference moves between sessions.** In this session lsaca came out at
   0.82x C (ahead of C); in the 1.3 session it was 1.24x C, and an earlier interleaved
   re-timing of a previous binary pair (9 runs each, alternating) measured 36.9 ms
   (lsaca) vs 39.0 ms (C) - roughly parity. Treat the fib row as "same ballpark", not as
   a stable speed ratio.

The constant argument n=30 does not explain the differences: probes with
`std::hint::black_box(30)` in Rust and a `volatile` n in C produced timings within a few
percent of the plain versions, so the compilers are not constant-folding the recursion. In
the generated lsaca program n is a runtime variable anyway.

### String building: runtime-bound, 2.4x C

`string_build` appends 1,000,000 small strings. lsaca needs 39.4 ms versus 16.7 ms for C
and 96.1 ms for Rust (Rust is slower than C here because it also allocates a temporary
`String` per token). In lsaca 1.1.0 the same program took 212.9 ms; the 1.2 runtime work
brought that down to 33.9 ms, and this session measures 39.4 ms - 5.4x faster than 1.1.0
and 2.4x faster than the Rust version. (C measured 27.0 ms in the 1.3 session and
16.7 ms here, so the ratio moved with the machine as well: compare within a row, not
across sessions.)

The remaining 2.4x gap to C is about what a `String` is in each implementation:

- The C program allocates one buffer of the exact final size up front and writes bytes
  into it. There are almost no function calls in the hot loop.
- At the lsaca level the program is `s.push_str(format!("{},", v))`. Each iteration
  builds a temporary formatted `String` and appends it through the runtime's string
  append function, which handles reference counting and geometric growth. The loop
  therefore performs one allocation plus one append per token; C's version performs none.
- The peak-memory row reflects the same design: the last growth step keeps the old and
  the new buffer alive at the same time (10.4 MiB total versus 7.4 MiB for C and Rust).

The measured 2.4x is a statement about the current runtime implementation, not about the
language: an API that let the caller append into a pre-sized buffer, or that inlined the
growth path, could close the rest of the gap.

### Vectors of struct values: now at parity

`vec_memory` pushes 1,000,000 two-word structs into a `Vec`. In this session lsaca
takes 23.4 ms and peaks at 18.8 MiB, versus 22.4 ms / 18.8 MiB in C and 23.9 ms / 18.9 MiB
in Rust (all three within run-to-run noise of each other; 1.2 measured 16.8 ms for lsaca
on the same program). In 1.1.0 it was 86.1 ms and 35.1 MiB.

Two runtime and code-generation changes explain the difference: the hot `push` path is
now a fast path that writes the value into the reserved slot directly instead of going
through a temporary element and a generic insertion helper, and `for p in points`
consumes the vector since 1.4, so the loop does not deep-copy the whole buffer.

`Vec` is a **move-only value type** in lsaca: moving it (assignment, argument, return)
moves the buffer, and `.clone()` deep-copies it. Since 1.3 the language also has
references, so a helper function can take `&Vec<T>`/`&mut Vec<T>` and look at or mutate
the vector in place without copying it, and since 1.4 mutable heap references work too.
The vector in this workload is built and consumed in one function without such a copy, so
the benchmark shows that the common path is as cheap as C. Shapes that do copy (see
`binary_trees` and the fairness notes) still pay for it, but the source now has to ask
for the copy.

### Sorting: 200,000 values, in-language algorithm

`sort_i64` sorts 200,000 pseudo-random values with an iterative quicksort written in the
language: explicit stack of index pairs, middle pivot, Hoare partition, manual swaps
through bounds-checked indexing. lsaca takes 41.4 ms versus 37.2 ms in C and 35.6 ms in
Rust, a 1.11x gap (1.2 measured near-parity: 35.7 vs 37.4 vs 33.2 ms). The C version uses
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
takes 16.1 ms versus 16.6 ms for C and 15.6 ms for Rust - parity in this session (1.2
measured 26.4 vs 18.4 vs 24.2 ms, a 1.44x gap). The inner loop indexes the two input
vectors and the output vector by computed positions (`a[i*n+k] * b[k*n+j]`), so every
access is bounds-checked in lsaca, while C and Rust index without a check. Bounds checks
of that shape are typically a few percent on a workload this small; the session-to-session
swing (and the fact that the whole program is 16 ms) means this row is dominated by
code-generation and session noise. The arithmetic itself is plain C code, which is why all
three implementations land in the same range.

### Binary trees: value semantics versus the borrowed traversal

`binary_trees` builds a perfect tree of 131,071 nodes and sums every payload through a
value-taking function (`fn sum_tree(tree: Option<Box<Node>>) -> i64`). C takes 25.1 ms and
peaks at 7.7 MiB; lsaca takes 677.3 ms and peaks at 101.1 MiB. This is by far the largest
gap in the suite, and it is a language-model effect rather than a codegen effect:

- In C the tree is `malloc`ed nodes with raw pointers: building and traversing it never
  duplicates anything.
- In Rust the tree is `Box<Node>`: moving a `Box` is moving a pointer.
- In lsaca 1.4, move-only values are moved, so building the tree and passing it into
  functions no longer duplicates it. But the traversal owns each node it matches and then
  needs a copy of its children: `Box::get()` returns an owned copy of the payload, and
  taking a child by value would move it out of a field, which is `E0507`, so the program
  clones the child subtrees explicitly with `.clone()`. Every level of the recursion
  therefore copies its subtree. The 27x runtime and the 13.2x peak-memory gap are the
  price of that value-semantics shape, and 1.4 makes the copies visible in the source
  instead of hiding them in implicit copies.

Version 1.4 ships a second workload on exactly the same tree: `binary_trees_refs` sums it
through references instead - `fn sum_tree(tree: &Option<Box<Node>>) -> i64`, matching
`Some(boxed)` and recursing on `&current.left` and `&current.right`. That traversal does
not copy the tree at all. It takes 35.8 ms and peaks at 19.9 MiB, so **the borrowed
traversal is 18.9x faster and uses 5.1x less peak memory than the value traversal on the
same tree, same nodes and same algorithm**. It is also 1.33x C's runtime and 2.6x its peak
memory - the first time this shape has come within a factor of two of C rather than a
factor of ten.

The borrowed version is not free of the runtime: it is still 2.6x C's peak memory, because
the tree is built and dropped out of reference-counted `Box` nodes. It does beat the
interpreters, though: Python takes 146.2 ms and Node.js 184.1 ms on the same borrowed
traversal, versus lsaca's 35.8 ms. Python's 111.9 ms on the by-value variant is still
faster than lsaca's 677.3 ms, because Python objects are references and aliasing is the
default there.

### String splitting and parsing

`string_split` builds a 194,500-byte comma-separated string, splits it into 50,000
tokens and parses each one. lsaca takes 31.7 ms versus 19.5 ms for C (1.62x) and 17.2 ms
for Rust. The result-string allocation and the per-token parse are both runtime calls;
the gap is in the same class as `string_build` but much smaller because the string is
short and the tokens are small.

### Compile times: direct C back end, not CMake

A cold lsaca build of a small program is 1.03-1.32 s in this session (1.15-1.72 s in the
1.3 session and 0.97-1.37 s in the 1.2.0 session). Two things dominate it: compiling the C
runtime translation units (about 86 KB of C in total) and compiling plus linking the
generated `main.c`; a single-file `gcc -O2` compile of the same program takes
0.27-0.42 s, so the runtime and the project overhead account for most of the difference. A
cached build is 20-36 ms because the content-addressed cache returns the previous
executable without invoking gcc at all.

In 1.1.0 the same cold build took 2.79-3.27 s because the pipeline included CMake
configuration and a Ninja build of the generated project. The 1.2 direct C compiler back
end (the default) skipped that layer, which made cold builds about 2.6-3.3x faster. The
old pipeline is still available with `--cmake` for comparison and for environments where
direct gcc invocation is not possible.

`compile_large` shows the scaling in generated-C size: 1,046,885 bytes (about 1.05 MB) of
C, 4.03 s cold, 324 ms cached - roughly 3x a small program's build time.

The answer to "lsaca compile speed" therefore depends on the cache: a first build is
slower than gcc by roughly 3-4x for small programs (mostly gcc on the runtime); rebuilding
an *unchanged* program is faster than either compiler, because the cache hit skips gcc
entirely.

### What 1.4 changed: ownership and moves

1.4 added ownership and move semantics: values of move-only types (`String`, `Vec<T>`,
`Rc<T>`, `Box<T>`, closures, and user types that contain one of those or implement
`Drop`) are moved on assignment, argument passing, returning and pattern matching instead
of being copied, and `T.clone()` is the explicit way to ask for a copy. The tree rows are
a controlled comparison of the two language models, because the tree, the node type, the
algorithm and the checksum are identical; only the traversal signature differs:

| workload | runtime (ms) | peak (MiB) | runtime vs C | peak vs C |
| --- | ---: | ---: | ---: | ---: |
| `binary_trees` (traverse by value) | 677.3 | 101.1 | 27.0x | 13.2x |
| `binary_trees_refs` (traverse through `&`) | 35.8 | 19.9 | 1.33x | 2.6x |
| C `binary_trees` (same tree) | 25.1 | 7.7 | 1.0x | 1.0x |

Both lsaca rows improved substantially against the 1.3 session: the by-value row went from
1216.7 ms / 150.1 MiB to 677.3 ms / 101.1 MiB (1.8x faster, 1.5x less peak memory), and
the borrowed row from 223.2 ms / 35.8 MiB to 35.8 ms / 19.9 MiB (6.2x faster, 1.8x less
peak memory). The C row barely moved (25.8 ms -> 25.1 ms), so the machine was in the same
class both times; the improvement is the language change. In 1.3 every assignment,
argument and struct-field initialiser copied its value, so even *building* the tree copied
subtrees at every level. In 1.4 building and passing the tree moves it, so the only copies
left are the ones the source asks for: the by-value traversal's `.clone()` calls, and none
at all in the borrowed traversal.

The rest of the suite is unchanged within session noise. For reference, the 1.2.0 session
measured 34.9/11.7/33.9/16.8 ms for `fib`/`loop_sum`/`string_build`/`vec_memory`;
the 1.3 session measured 44.2/13.9/45.2/30.7 ms and this 1.4 session 35.8/12.7/39.4/23.4
ms. Cross-session absolute numbers are not comparable (see the noise note). The earlier
1.1.0-to-1.2.0 improvements (`string_build` 6.3x faster, `vec_memory` 5.1x faster, direct
C back end instead of CMake) remain in the code base.

### Python and Node.js: start-up, interpretation and JIT

Python is 4.11-101.92x slower than C on these workloads. The widest gap is
`matrix_multiply` (1689.4 ms vs 16.6 ms, 101.9x), where 4.1 million multiply-adds run in
nested loops; next is `loop_sum` (1147.1 vs 12.3 ms, 93.6x), where CPython executes 8
million integer additions in a bytecode loop; the narrowest is `string_split`
(80.3 vs 19.5 ms, 4.11x), where most of the work happens in C-level string methods, and
`binary_trees`/`binary_trees_refs` (4.46x/5.44x), where Python's reference semantics pay
off (Python is faster than lsaca on the by-value tree row and slower on the borrowed row,
as noted above).

Node.js lands between: 4.65-14.16x C. V8's JIT does well on the simple loops
(`mandelbrot` 162.2 ms, 4.65x; `sieve` 201.8 ms, 5.85x) but the numbers include V8
start-up on every run (each measured run is a fresh process), and its memory floor is
visible in the memory table (42.8-60.0 MiB for the small workloads versus 4 MiB for native
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
   | `binary_trees` | `Option<Box<Node>>` recursive struct; the by-value traversal clones child subtrees with `.clone()` (moving a field out is `E0507`) | `malloc`ed nodes + raw pointers | `Option<Box<Node>>` if let / match | class with `__slots__` | plain objects |
   | `binary_trees_refs` | same tree, summed via `&Option<Box<Node>>` (no copy) | same program as `binary_trees` (C uses pointers anyway) | same program as `binary_trees` (moving a `Box` is a pointer move) | same program | same program |
   | `sieve` | `vec![0; limit + 1]` of `i64` | `calloc`ed `int64_t` array | `vec![0i64; n]` | list of ints | `Float64Array` |

   These are all reasonable code, but they do different amounts of low-level work, and
   that is exactly what the benchmarks are measuring.

2. **Contiguous values versus per-element objects.** C, Rust and lsaca store `Point`
   values contiguously in the vector/array; Python tuples and JS objects are individual
   heap objects referenced from the list/array. The memory rows for `vec_memory` are
   largely a consequence of that representation difference, not only of the runtime. The
   same inversion explains why Python (111.9 ms) beats lsaca on `binary_trees`
   (677.3 ms): reference semantics make aliasing cheap, while lsaca's by-value traversal
   copies subtrees. On the borrowed traversal the order reverses - lsaca 35.8 ms versus
   Python 146.2 ms - because not copying beats cheap aliasing.

3. **Value semantics: moves, not implicit copies.** Since 1.4 an lsaca `Vec`, `Box` or
   `String` is a move-only value: moving it (assignment, passing it by value, returning
   it) moves the payload, and `.clone()` is the explicit way to copy it. That is closer to
   Rust's move model than to the implicit copying of 1.3, but the reference checker is
   conservative: `&T`/`&mut T` are non-owning pointers with an alias check, not
   lifetime-tracked borrows, and storing a reference in an aggregate is still rejected.
   Since 1.3 the language has had references, which is how `binary_trees_refs` traverses
   the tree without copying it; the by-value `binary_trees` variant has to clone each
   child subtree explicitly, and that difference is the measured value of reference
   support. Earlier versions avoided copies where they could - `Vec::with_capacity(n)`
   bound to a `let`, and `for x in vec` without duplicating the buffer (hence the
   `vec_memory` parity) - and 1.4 keeps those by moving instead of deep-copying.

4. **lsaca compiler limitations at measurement time.** The language has no generics and
   no `Vec` sort method, and no unsigned/small integer types in these programs, so the
   workloads stay deliberately simple. (`binary_trees_refs` exercises references, the
   only place they appear in the suite; the programs were migrated to 1.4 without
   changing their algorithms - the by-value tree traversal now spells out the `.clone()`
   calls that 1.3 performed implicitly.) In particular `sort_i64` does not call a library
   sort in any language, and the string workloads use only the documented `String` API.
   These numbers describe one young compiler version (1.4.0), not the language's
   potential.

5. **What the lsaca release build does.** Release builds wrap `+ - *` (the generated C
   uses explicit unsigned casts), so there are no overflow checks on those operations.
   Division and remainder always go through the runtime's checked helpers because
   division by zero panics even in release; `string_build` performs one million
   `i % 1000` operations, `sort_i64` 200,000 LCG remainders and `matrix_multiply`'s
   fill loop uses `%` as well. Vector indexing is bounds-checked on every access, which
   is the main extra work in `matrix_multiply`'s inner loop (that row still comes out at
   parity in this session, so the checks are not a large cost at this size). The runtime's
   reference-count and copy-on-write checks are per string call. Since 1.4, moving a value
   is a pointer move with no allocation, a `.clone()` performs exactly the deep copy the
   source asks for, and `Drop` hooks run when a heap value is released - so ownership adds
   no per-value bookkeeping to these loops beyond the reference count the runtime already
   maintained.

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
   session versus 0.31-0.42 s here, while gcc stayed at 0.27-0.50 s. The medians absorb
   individual bad samples: the 1.3 run contained one Python `sieve` sample at 2.48 s
   against a 772.5 ms median, and the 1.2.0 run one Python `string_build` sample at
   5.38 s; median and minimum are unaffected. The earlier 1.3 session also discarded a
   first attempt that was 1.2-1.6x slower across every language, C and Rust included - a
   uniform slowdown means a busy machine, not a slow compiler. This 1.4 run recorded no
   warnings: no sample needed re-measuring.

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

# 2. Run the full suite (roughly 3-5 minutes, depending on the machine).
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
