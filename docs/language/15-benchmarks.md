# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
intuition: the same ten workloads are implemented in lsaca, C, Rust, Python and Node.js,
built with each language's normal toolchain, run on the same machine, and checked against
a shared expected output. An eleventh, larger source is measured for compile time only.

The numbers below were measured on **2026-10-25** with the repository's harness,
`scripts/bench.py`, on the reference machine described in
[Environment](#environment). The raw data (including every individual run, not just the
medians) is in [benchmarks/results/latest.json](../../benchmarks/results/latest.json);
the harness also writes a machine-generated subset of these tables to
[benchmarks/results/latest.md](../../benchmarks/results/latest.md). Instructions for
reproducing the run are at the end of this page.

The short version for lsaca 1.2.0:

- **Scalar loops, sorting and floating-point code are at parity with C.** `loop_sum`
  is 11.7 ms in lsaca versus 10.9 ms in C, `sort_i64` is 35.7 versus 37.4 ms,
  `sieve` is 50.6 versus 50.1 ms and `mandelbrot` is 40.6 versus 40.7 ms.
- **`vec_memory` is now at parity too**: 16.8 ms and 18.8 MiB peak versus C's 17.1 ms
  and 18.8 MiB. In 1.1.0 the same workload took 86.1 ms and 35.1 MiB, so this is the
  largest improvement in 1.2 (about 5x faster, roughly half the peak memory).
- **`string_build` is 6.3x faster than in 1.1.0** (33.9 ms versus 212.9 ms) but is still
  2.2x C and 2x Rust: the remaining cost is the runtime's per-token string path, not the
  generated C.
- **`binary_trees` is the one structural outlier**: 1261 ms and 149.7 MiB versus 27.8 ms
  and 7.6 MiB in C, because `Box<T>` and `Vec<T>` are value types whose copies
  deep-copy their payloads (see the analysis below).
- **A cold build now takes 0.97-1.37 s** (it was 2.8-3.3 s in 1.1.0, which used CMake);
  the default direct C compiler back end emits C and drives gcc itself. A cached build is
  19-31 ms. The 4,511-line `compile_large` source (1.30 MB of generated C) takes 4.67 s
  cold and 165 ms cached.
- **Interpreters are 4.4-103x slower than C** on these workloads; Python's numbers also
  carry interpreter start-up on every run.

## Workloads

Ten small programs, each of which prints a single `checksum=...` line, plus a large
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

All 51 (workload, language) combinations were built and run, and every measured run
matched its expected checksum line. The machine-readable result says
`validation: "all measured runs matched the expected checksum"`. `compile_large` is
built but not executed, so it has no runtime or memory rows. Because the checksums cover
the whole workload, an implementation that silently skipped work would fail the run
instead of appearing "fast".

The result file also carries one warning about the measurement itself: the first
cold-build sample of `string_build`/lsaca was taken while an unrelated background build
was running on the machine and came out at 32.1 s instead of about 1 s. That single cell
was re-measured with identical harness settings (1.053 s) and the replacement is used in
the tables; every other number comes from the primary run.

## Environment

Everything in the table was collected by the harness at run time and is stored in
`benchmarks/results/latest.json`, together with the individual run times.

| field | value |
| --- | --- |
| Measured at | 2026-10-25 12:52:17 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 1.2.0, repository commit `6697c87`, compiler sha256 prefix `43c0484d14130f51` |
| lsaca build path | `lsaca build -r`, default direct C compiler back end (gcc 15.2.0, `-O2 -DNDEBUG`); the old CMake + Ninja path is available as `--cmake` |
| C compiler | gcc 15.2.0 (MSYS2 UCRT64, Rev14, built by MSYS2 project) |
| Rust compiler | rustc 1.99.0 (`b940084d7 2026-09-28`) |
| Python | 3.14.4 |
| Node.js | v24.14.1 |
| Harness | `scripts/bench.py`, 11 programs (10 executed), 1 warm-up + 7 runs, 3 compile repeats, peak-memory sampling on |

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

The "lsaca emit-c + gcc" column is only filled when the harness runs in fallback mode
(see the fairness notes); in this official run the default build back end was used and
that column is not measured.

Observations:

- A cold lsaca build is **0.97-1.37 s** (4.67 s for the 4,511-line `compile_large`).
  Most of that is gcc: compiling the generated `main.c` together with the four runtime
  translation units and linking them (a single-file gcc compile of the same workload is
  0.25-0.45 s). The front end itself is a small part of the measurement - the C emission
  step is not separately recorded but is a few tens of milliseconds.
- `compile_large` shows the scaling: 4,511 lines / 450 functions produce 1.30 MB of
  generated C and 4.67 s of cold build time, roughly 3.5-4x a small program's build. The
  build time therefore grows with the generated C, as expected for "run gcc on C".
- A cached lsaca build is **19-31 ms** for the small programs (165 ms for
  `compile_large`). That is the content-addressed cache hit: the compiler hashes the
  generated program and returns the existing executable rather than invoking gcc. In
  practice an edit to the `.ls` source invalidates the cache and pays the full cold cost
  again; there is no incremental compile yet.
- The C and Rust columns (0.25-0.45 s) are single-file compiles with no project setup;
  they are the right baseline for "front end plus codegen", not for the whole
  emit-compile-link pipeline.

### Runtime

Milliseconds, **median (minimum)** of 7 measured runs after 1 warm-up. Lower is better.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 34.9 (31.7) | 11.7 (10.7) | 33.9 (33.0) | 16.8 (16.3) | 35.7 (35.3) | 26.4 (19.0) | 1261.4 (1146.8) | 26.5 (25.6) | 50.6 (42.2) | 40.6 (38.1) |
| C (gcc -O2) | 30.6 (28.9) | 10.9 (10.3) | 15.2 (14.7) | 17.1 (16.8) | 37.4 (34.8) | 18.4 (16.4) | 27.8 (22.9) | 20.2 (17.6) | 50.1 (45.7) | 40.7 (38.9) |
| Rust (rustc -O) | 14.5 (14.0) | 14.5 (11.4) | 68.3 (65.0) | 18.4 (17.8) | 33.2 (32.6) | 24.2 (22.8) | 28.6 (24.4) | 20.7 (17.2) | 46.7 (42.0) | 42.1 (38.2) |
| Python 3 | 1056.4 (1015.3) | 1024.0 (1010.3) | 423.7 (414.9) | 576.8 (544.9) | 864.4 (709.4) | 1887.4 (1512.2) | 132.8 (114.3) | 92.9 (87.5) | 687.4 (623.1) | 3366.2 (3311.7) |
| Node.js | 270.9 (238.4) | 143.3 (140.4) | 210.2 (202.5) | 260.0 (242.0) | 246.0 (208.4) | 221.4 (193.8) | 197.9 (161.1) | 183.1 (155.7) | 222.2 (217.5) | 206.9 (184.5) |

The same numbers as a ratio to the C time (median / C median). Below 1.0 means faster
than C:

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 1.14x | 1.08x | 2.23x | 0.99x | 0.95x | 1.44x | 45.42x | 1.31x | 1.01x | 1.00x |
| Rust | 0.47x | 1.33x | 4.49x | 1.08x | 0.89x | 1.31x | 1.03x | 1.02x | 0.93x | 1.03x |
| Python 3 | 34.50x | 94.17x | 27.81x | 33.80x | 23.08x | 102.62x | 4.78x | 4.60x | 13.73x | 82.64x |
| Node.js | 8.85x | 13.18x | 13.80x | 15.24x | 6.57x | 12.04x | 7.13x | 9.06x | 4.44x | 5.08x |

### Peak memory

MiB of peak working set, separate run. Lower is better. Remember that this is total
process footprint: it includes the runtime, the interpreter/VM and its heap.

| language | fib | loop_sum | string_build | vec_memory | sort_i64 | matrix_multiply | binary_trees | string_split | sieve | mandelbrot |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 18.8 | 6.6 | 4.4 | 149.7 | 11.1 | 18.8 | 4.0 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 | 5.1 | 4.3 | 7.6 | 4.0 | 18.8 | 4.0 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.4 | 18.9 | 5.2 | 4.5 | 7.7 | 4.4 | 18.9 | 4.1 |
| Python 3 | 11.0 | 10.9 | 74.0 | 143.8 | 20.6 | 13.6 | 21.0 | 17.2 | 26.2 | 11.0 |
| Node.js | 42.8 | 45.7 | 89.3 | 117.2 | 50.6 | 46.8 | 56.1 | 50.6 | 59.9 | 44.6 |

For workloads with no large data structures (`fib`, `loop_sum`, `mandelbrot`) every
native language sits at the 4 MiB floor of the process image. Where the data is large:
`vec_memory` allocates 16 MB of `Point` values and both lsaca and C peak at 18.8 MiB
(1.2 stores the vector in place, whereas 1.1 duplicated it and peaked at 35.1 MiB);
`sieve` keeps a 2,000,000-element `i64` flag vector, 16 MB, and peaks at 18.8 MiB;
`string_build` allocates a ~3.9 MB string and adds a temporary per token, hence
10.6 MiB versus 7.3 MiB for C. `binary_trees` is the outlier: 149.7 MiB for 131,071
small nodes, because copying a `Box<Node>` copies the whole subtree.

## Analysis

### Scalar integer loops: parity with C

`loop_sum` is the cleanest comparison: 8,000,000 iterations of `sum += i` and nothing
else. lsaca takes 11.7 ms, C 10.9 ms, Rust 14.5 ms - all within a few percent of each
other. In release mode the generated C lowers integer additions to plain C arithmetic
(the wrapping semantics are expressed as explicit `uint64_t` casts, so the compiler does
not need to keep overflow checks), and GCC -O2 turns the loop into comparable machine code
to a hand-written C loop.

`sieve` (50.6 vs 50.1 ms) and `mandelbrot` (40.6 vs 40.7 ms) tell the same story for an
array-heavy loop and for floating-point arithmetic: when the work does not need the
runtime's help, lsaca generates C that performs like C. This is the strongest evidence
that the C backend is not intrinsically slow.

### Recursion-heavy calls: fib

The recorded session has lsaca at 34.9 ms, C at 30.6 ms and Rust at 14.5 ms for the same
recursive algorithm. Two things matter here, and only one of them is about lsaca:

1. **C and Rust differ by 2.1x on identical source-level algorithms.** That is a
   code-generation effect between GCC 15.2 and LLVM (via rustc 1.99.0) on a
   self-recursive function, not a language-semantics effect. Any single workload can
   show a compiler gap of this size; it should not be read as "Rust is twice as fast".
2. **The lsaca / C difference is noisier than the other workloads.** The recorded session
   shows lsaca at 1.14x C, and an earlier interleaved re-timing of a previous binary pair
   (9 runs each, alternating) measured 36.9 ms (lsaca) vs 39.0 ms (C) - roughly parity.
   Treat the fib row as "same ballpark", not as a stable speed ratio.

The constant argument n=30 does not explain the differences: probes with
`std::hint::black_box(30)` in Rust and a `volatile` n in C produced timings within a few
percent of the plain versions, so the compilers are not constant-folding the recursion. In
the generated lsaca program n is a runtime variable anyway.

### String building: still runtime-bound, 6.3x faster than 1.1.0

`string_build` appends 1,000,000 small strings. lsaca needs 33.9 ms versus 15.2 ms for C
and 68.3 ms for Rust (Rust is slower than C here because it also allocates a temporary
`String` per token). In lsaca 1.1.0 the same program took 212.9 ms, so 1.2's runtime work
made this workload 6.3x faster.

The remaining 2.2x gap to C is about what a `String` is in each implementation:

- The C program allocates one buffer of the exact final size up front and writes bytes
  into it. There are almost no function calls in the hot loop.
- At the lsaca level the program is `s.push_str(format!("{},", v))`. Each iteration
  builds a temporary formatted `String` and appends it through the runtime's string
  append function, which handles reference counting and geometric growth. The loop
  therefore performs one allocation plus one append per token; C's version performs none.
- The peak-memory row reflects the same design: the last growth step keeps the old and
  the new buffer alive at the same time (10.6 MiB total versus 7.3-7.4 MiB for C and
  Rust).

The measured 2.2x is a statement about the current runtime implementation, not about the
language: an API that let the caller append into a pre-sized buffer, or that inlined the
growth path, could close the rest of the gap.

### Vectors of struct values: now at parity

`vec_memory` pushes 1,000,000 two-word structs into a `Vec`. In 1.2 lsaca takes 16.8 ms
and peaks at 18.8 MiB, versus 17.1 ms / 18.8 MiB in C and 18.4 ms / 18.9 MiB in Rust. In
1.1.0 it was 86.1 ms and 35.1 MiB.

Two runtime and code-generation changes explain the difference: the hot `push` path is
now a fast path that writes the value into the reserved slot directly instead of going
through a temporary element and a generic insertion helper, and iterating a vector with
`for p in points` no longer deep-copies the whole buffer.

`Vec` is still a **value type** in lsaca: an explicit copy of a vector deep-copies its
buffer, because the language has no references yet. The vector in this workload is built
and consumed without such a copy, so the benchmark shows that the common path is now as
cheap as C. Shapes that do copy (see `binary_trees` and the fairness notes) still pay for
it.

### Sorting: 200,000 values, in-language algorithm

`sort_i64` sorts 200,000 pseudo-random values with an iterative quicksort written in the
language: explicit stack of index pairs, middle pivot, Hoare partition, manual swaps
through bounds-checked indexing. lsaca takes 35.7 ms versus 37.4 ms in C and 33.2 ms in
Rust. The C version uses the same algorithm and the same explicit stack; the Rust version
uses `swap` on a mutable slice.

No language uses a library sort in this workload (lsaca has no `Vec::sort`), so this is a
fair "how expensive are the language's primitives in a write-heavy loop" comparison, and
the answer is: at parity with C, even with bounds checks on every index access. A
library `qsort`/`sort_unstable` in C or Rust would be faster than any of these numbers.

### Matrix multiplication: bounds checks in the inner loop

`matrix_multiply` multiplies two 160x160 `i64` matrices stored as flat vectors. lsaca
takes 26.4 ms versus 18.4 ms for C and 24.2 ms for Rust, a 1.44x gap. The inner loop
indexes the two input vectors and the output vector by computed positions
(`a[i*n+k] * b[k*n+j]`), so every access is bounds-checked in lsaca, while C and Rust
index without a check. That is the honest cost of the safe default; the arithmetic itself
is plain C code, which is why Rust is close to C and lsaca is not far behind.

### Binary trees: the cost of value semantics

`binary_trees` builds a perfect tree of 131,071 nodes and sums every payload. C takes
27.8 ms and peaks at 7.6 MiB; lsaca takes 1261.4 ms and peaks at 149.7 MiB. This is by far
the largest gap in the suite, and it is a language-model effect rather than a codegen
effect:

- In C the tree is `malloc`ed nodes with raw pointers: building and traversing it never
  duplicates anything.
- In Rust the tree is `Box<Node>`: moving a `Box` is moving a pointer.
- In lsaca, `Box<T>` is a **value type**: copying the box copies its payload, and the
  payload of a `Node` contains two boxes, so copying a node copies its whole subtree,
  recursively. Recursively building the tree and reading `boxed.get()` during traversal
  therefore duplicate nodes over and over. The 45x runtime and the 20x peak-memory gap
  are the cost of that model, not of the C backend.
- Interestingly Python (132.8 ms) is faster than lsaca here, because Python objects are
  references: aliasing is the default there, which is exactly what lsaca's value
  semantics forbids. This row is the clearest remaining structural difference in the
  suite, and the main reason to expect reference/borrow support in a future version.

### String splitting and parsing

`string_split` builds a 194,500-byte comma-separated string, splits it into 50,000
tokens and parses each one. lsaca takes 26.5 ms versus 20.2 ms for C (1.31x) and 20.7 ms
for Rust. The result-string allocation and the per-token parse are both runtime calls;
the gap is in the same class as `string_build` but much smaller because the string is
short and the tokens are small.

### Compile times: direct C back end, not CMake

A cold lsaca build of a small program is 0.97-1.37 s. Two things dominate it: compiling
the four C runtime translation units (about 86 KB of C in total) and compiling plus
linking the generated `main.c`; a single-file `gcc -O2` compile of the same program
takes 0.25-0.45 s, so the runtime and the project overhead account for most of the
difference. A cached build is 19-31 ms because the content-addressed cache returns the
previous executable without invoking gcc at all.

In 1.1.0 the same cold build took 2.79-3.27 s because the pipeline included CMake
configuration and a Ninja build of the generated project. The 1.2 direct C compiler back
end (the default) skipped that layer, which made cold builds about 2.6-3.3x faster. The
old pipeline is still available with `--cmake` for comparison and for environments where
direct gcc invocation is not possible.

The answer to "lsaca compile speed" therefore depends on the cache: a first build is
slower than C or Rust by roughly 2.5-4x for small programs (mostly gcc on the runtime);
rebuilding an *unchanged* program is faster than either, because the cache hit skips gcc
entirely. `compile_large` shows the scaling in generated-C size: 1.30 MB of C,
4.67 s cold, 165 ms cached.

### What changed since 1.1.0

| workload | 1.1.0 runtime (ms) | 1.2.0 runtime (ms) | 1.1.0 peak (MiB) | 1.2.0 peak (MiB) |
| --- | ---: | ---: | ---: | ---: |
| fib | 30.6 | 34.9 | 4.0 | 4.0 |
| loop_sum | 11.4 | 11.7 | 4.0 | 4.0 |
| string_build | 212.9 | 33.9 | 10.6 | 10.6 |
| vec_memory | 86.1 | 16.8 | 35.1 | 18.8 |
| cold build (small programs) | 2.79-3.27 s | 0.97-1.37 s | - | - |
| cached build (small programs) | 0.017-0.019 s | 0.019-0.031 s | - | - |

The 1.1.0 and 1.2.0 sessions are different runs of a shared machine, and the v1.1 table
covered only four workloads, so the comparison is indicative rather than controlled: the
`fib` and `loop_sum` differences are within noise, while `string_build` (6.3x),
`vec_memory` (5.1x) and the cold-build time (2.6-3.3x) are far outside it.

### Python and Node.js: start-up, interpretation and JIT

Python is 4.6-102.6x slower than C on these workloads. The widest gap is
`matrix_multiply` (1887 ms vs 18.4 ms), where CPython executes 4.1 million multiply-adds
in nested loops; the narrowest is `string_split` (4.6x), where most of the work happens
in C-level string methods, and `binary_trees` (4.8x), where Python's reference semantics
pay off (Python is faster than lsaca there, as noted above).

Node.js lands between: 4.4-15.2x C. V8's JIT does well on the simple loops (`sieve`
222 ms, 4.4x) but the numbers include V8 start-up on every run (each measured run is a
fresh process), and its memory floor is visible in the memory table (43-51 MiB for the
small workloads versus 4 MiB for native code).

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
   | `sieve` | `vec![0; limit + 1]` of `i64` | `calloc`ed `int64_t` array | `vec![0i64; n]` | list of ints | `Float64Array` |

   These are all reasonable code, but they do different amounts of low-level work, and
   that is exactly what the benchmarks are measuring.

2. **Contiguous values versus per-element objects.** C, Rust and lsaca store `Point`
   values contiguously in the vector/array; Python tuples and JS objects are individual
   heap objects referenced from the list/array. The memory rows for `vec_memory` are
   largely a consequence of that representation difference, not only of the runtime. The
   same inversion explains why Python beats lsaca on `binary_trees`: reference semantics
   make aliasing cheap, and lsaca's value-semantics `Box` copies subtrees instead.

3. **Value semantics: copies, not references.** An lsaca `Vec`/`Box` is a value type.
   Copying it (assignment, passing it by value, or an explicit copy) deep-copies the
   payload, unlike Rust's move/borrow model or C's explicit pointers. 1.2 avoids the
   copies where it can: `Vec::with_capacity(n)` bound to a `let` and `for x in vec`
   iteration no longer duplicate the buffer (hence the `vec_memory` parity). Recursive
   `Box` structures still copy on `get()`, which is the `binary_trees` number. The
   language has no references yet; this is the main known structural cost.

4. **lsaca compiler limitations at measurement time.** The language has no generics and
   no `Vec` sort method, no references, and no unsigned/small integer types in these
   programs, so the workloads stay deliberately simple. In particular `sort_i64` does
   not call a library sort in any language, and the string workloads use only the
   documented `String` API. These numbers describe one young compiler version (1.2.0),
   not the language's potential.

5. **What the lsaca release build does.** Release builds wrap `+ - *` (the generated C
   uses explicit unsigned casts), so there are no overflow checks on those operations.
   Division and remainder always go through the runtime's checked helpers because
   division by zero panics even in release; `string_build` performs one million
   `i % 1000` operations, `sort_i64` 200,000 LCG remainders and `matrix_multiply`'s
   fill loop uses `%` as well. Vector indexing is bounds-checked on every access, which
   is visible in `matrix_multiply`'s inner loop. The runtime's reference-count and
   copy-on-write checks are per string call.

6. **Compiler and flags.** gcc 15.2.0 (MSYS2 UCRT64) and rustc 1.99.0, each with the
   single normal optimisation flag (`-O2` / `-O`). No LTO, PGO, `-march=native` or
   target-cpu tuning for any language. The lsaca runtime is compiled by the same gcc with
   `-O2 -DNDEBUG`. The comparison is of standard builds on one Windows machine, not of
   tuned builds on Linux.

7. **Noise, statistics and a shared machine.** 7 measured runs, median (and minimum)
   reported. This is a laptop with P/E cores, power management and background activity,
   and the benchmark suite was run while other builds were happening on the same machine.
   A second full run about six minutes later was 10-25% slower across the board (e.g.
   `compile_large` 4.67 vs 5.83 s, `fib`/lsaca 34.9 vs 38.4 ms, Python `mandelbrot`
   3366 vs 4057 ms), while the peak-memory numbers were stable within about 1% (the
   largest change was Node.js `vec_memory`, 117.2 to 121.2 MiB, about 3.5%). Treat
   single-digit percentage differences as noise, and do not compare this page's absolute
   numbers with a differently loaded machine. The medians also absorb individual bad
   samples: the recorded run contains e.g. one Node.js `fib` run at 1.0 s, one Python
   `vec_memory` run at 15.5 s and one Python `string_build` run at 2.57 s; the median and
   minimum are unaffected. The one cold-build sample that was hit by a background build at
   the start of the run (32.1 s) was re-measured and replaced, as described under
   [Validation](#validation).

8. **Peak memory is working set, not allocation.** It includes the executable, runtime
   metadata and allocator arena. For Node.js the floor is 43-51 MiB and for Python
   ~11 MiB, so their entries measure the interpreter as much as the data. Sampling can
   miss very short spikes.

9. **Start-up is included in every runtime number.** Each measured run is a new process:
   dynamic loading, runtime initialisation and (for Python/Node.js) interpreter/VM start-up
   are all part of the time. For a 12 ms workload like `loop_sum` in C, start-up is a
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

# 2. Run the full suite (roughly 3-5 minutes on the reference machine).
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
