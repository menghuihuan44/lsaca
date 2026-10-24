# Benchmarks

How fast is a program written in lsaca? This page answers with measurements rather than
intuition: the same four workloads are implemented in lsaca, C, Rust, Python and Node.js,
built with each language's normal toolchain, run on the same machine, and checked against
a shared expected output.

The numbers below were measured on **2026-10-24** with the repository's harness,
`scripts/bench.py`, on the reference machine described in
[Environment](#environment). The raw data (including every individual run, not just the
medians) is in [benchmarks/results/latest.json](../../benchmarks/results/latest.json);
the harness also writes a machine-generated subset of these tables to
[benchmarks/results/latest.md](../../benchmarks/results/latest.md). Instructions for
reproducing the run are at the end of this page.

The short version for lsaca 1.1.0:

- **Scalar integer loops are at parity with C.** `loop_sum` is 11.4 ms in lsaca and
  10.9 ms in C.
- **Recursion-heavy code is in the same ballpark as C**: `fib` measured 30.6 ms in both
  lsaca and C in this run, while Rust's LLVM backend is clearly ahead on this particular
  shape (see the [honest caveats](#fairness-notes-and-caveats); the fib row is noisier
  than the others).
- **Heavy allocation and string building are 4.7-14.4x slower than C**, because of runtime
  design (refcounted strings with copy-on-write checks, temporary allocations, generic
  per-push vector helpers and value-semantics copies) rather than C code generation
  quality.
- **Compiling a program cold takes 2.8-3.3 s**, almost all of it CMake configuration plus
  compiling the generated C and the runtime; a warm, cached build is 17-20 ms.
- **Interpreters are 7.9-91x slower than C** on these workloads; Python's numbers also
  carry interpreter start-up on every run.

## Workloads

Four small programs, each of which prints a single `checksum=...` line. They were chosen
to be simple enough that every language can express the same algorithm without exotic
workarounds, while still stressing different parts of a runtime.

| workload | what it stresses | exact workload | expected stdout |
| --- | --- | --- | --- |
| `fib` | non-tail recursion and calls | `fib(30)` computed 10 times and summed; `n` is a constant | `checksum=8320400` |
| `loop_sum` | tight scalar integer loop | sum of 1..=8,000,000 | `checksum=32000004000000` |
| `string_build` | building a string from many small pieces | append 1,000,000 decimal tokens `i % 1000`, each followed by a comma; checksum is the final byte length and the sum of the token values | `checksum=3890000,499500000` |
| `vec_memory` | allocation volume and peak memory | build a vector of 1,000,000 `Point { x: i64, y: i64 }` values with `x = i`, `y = 2*i`; checksum over `x + y` | `checksum=1499998500000` |

The sizes are deliberate. `loop_sum` uses 8,000,000 iterations so that Python takes about
one second: long enough to measure above start-up noise, short enough that seven runs per
workload keep the whole suite under five minutes. `fib` uses n=30 for the same reason.

Sources: [benchmarks/programs/](../../benchmarks/programs/), one file per workload and
language (`.ls`, `.c`, `.rs`, `.py`, `.js`).

## What is measured

### Compile time

Wall-clock time of one build command, median of 3 repetitions (1 in `--quick` mode):

- **lsaca, cold build**: `lsaca build -r <source> -o <outdir>` with a **fresh**
  `LSACA_CACHE_DIR` for each measurement. This is the full user-facing path: emit C,
  CMake configure, compile `main.c` and the runtime C file, link. The fresh cache
  directory prevents a previous build from being reused.
- **lsaca, cached build**: the same command immediately afterwards, with the cache
  directory retained. The compiler keys its build on a content hash, finds the previously
  built executable and returns it without running CMake or the C compiler.
- **C**: `gcc -O2 <source> -o <exe>`.
- **Rust**: `rustc -O <source> -o <exe>`.
- **Python / Node.js**: not applicable - these languages have no ahead-of-time compile
  step in this suite. Their start-up cost is part of the runtime numbers instead.

Not included in any compile figure: building the lsaca compiler itself
(`cargo build --release -p lsaca-cli`), or the one-time CMake toolchain setup on a
brand-new machine. The harness sets `TEMP`/`TMP`/`TMPDIR` to a directory inside
`target/bench-tmp` and puts the usual toolchain directories on `PATH`
(`C:\msys64\ucrt64\bin`, `C:\Program Files\CMake\bin`, `~/.cargo/bin` on Windows)
so the numbers do not depend on the caller's shell configuration.

### Runtime

Each program runs as a **fresh child process**. The harness does 1 unmeasured warm-up run
and then 7 measured runs (3 in `--quick` mode), and reports the **median** and the
**minimum** of the measured runs. Every measured run's stdout is validated against the
expected line; a mismatch or a non-zero exit aborts the whole run.

- Process start-up (loader, runtime initialisation, interpreter/VM startup) is included.
  It is small for native programs and not small for Python/Node.js, which is part of what
  the table shows.
- No CPU pinning, no warm-up beyond the one warm-up run; the machine is a normal laptop
  with power management enabled (see the caveats).

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

All 20 (workload, language) combinations were built and run, and every measured run
matched its expected checksum line. The machine-readable result says
`validation: "all measured runs matched the expected checksum"` and contains no warnings.
Because the checksum covers the whole workload, an implementation that silently skipped
work would fail the run instead of appearing "fast".

## Environment

Everything in the table was collected by the harness at run time and is stored in
`benchmarks/results/latest.json`, together with the individual run times.

| field | value |
| --- | --- |
| Measured at | 2026-10-24 21:57:52 +08:00 (harness timestamp at run start) |
| Machine | laptop, 12th Gen Intel Core i7-1260P (4 performance + 8 efficiency cores, 16 logical CPUs), 15.63 GiB RAM |
| OS | Windows 11, build 10.0.26300, AMD64 |
| lsaca | 1.1.0, repository commit `586f469`, compiler sha256 prefix `efe561efae9bd03a` |
| lsaca build path | `lsaca build -r` (official mode: emit C, CMake 4.1.0 + Ninja 1.13.2, Release build) |
| C compiler | gcc 15.2.0 (MSYS2 UCRT64, Rev14, built by MSYS2 project) |
| Rust compiler | rustc 1.99.0 (`b940084d7 2026-09-28`) |
| Python | 3.14.4 |
| Node.js | v24.14.1 |
| Harness | `scripts/bench.py`, 1 warm-up + 7 runs, 3 compile repeats, peak-memory sampling on |

The lsaca executable used for the run is a copy of `target/release/lsaca.exe`
(`target/bench-tools/lsaca.exe`); the harness records its SHA-256 prefix so a result can
be tied to a specific compiler binary. The compiler does not need to be rebuilt for a
benchmark run.

## Results

### Compile time

Seconds, median of 3 runs. Lower is better. "cold" and "cached" are two different
questions - a first build versus a rebuild with the compiler's build cache already warm.

| program | lsaca cold build | lsaca cached build | gcc -O2 | rustc -O | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| fib | 3.270 | 0.019 | 0.305 | 0.274 | n/a | n/a |
| loop_sum | 2.874 | 0.018 | 0.248 | 0.257 | n/a | n/a |
| string_build | 2.787 | 0.017 | 0.261 | 0.305 | n/a | n/a |
| vec_memory | 2.917 | 0.018 | 0.260 | 0.356 | n/a | n/a |

Observations:

- A cold lsaca build is **2.8-3.3 s** (2.79-3.27 s across the four programs), about 10-12x
  a single-file gcc compile. That time is not the lsaca front end: it is the fixed cost of
  the current pipeline - CMake configuration of a generated C project followed by compiling
  `main.c` together with the roughly 90 KB C runtime source, then linking.
- A cached lsaca build is **17-20 ms**. That is the content-addressed cache hit: the
  compiler hashes the generated program and returns the existing executable rather than
  running CMake again. The compile time of a rebuild is therefore close to the compile
  time of the C and Rust single-file builds, but only once the cache is warm.
- The C and Rust columns (0.25-0.36 s) are single-file compiles with no project setup;
  they are the right baseline for "front end plus codegen", not for the whole CMake
  pipeline.

### Runtime

Milliseconds, **median (minimum)** of 7 measured runs after 1 warm-up. Lower is better.

| language | fib | loop_sum | string_build | vec_memory |
| --- | ---: | ---: | ---: | ---: |
| lsaca | 30.6 (29.1) | 11.4 (10.9) | 212.9 (210.4) | 86.1 (83.5) |
| C (gcc -O2) | 30.6 (29.1) | 10.9 (10.8) | 14.8 (14.2) | 18.4 (17.1) |
| Rust (rustc -O) | 15.7 (15.1) | 12.0 (10.8) | 76.3 (69.5) | 24.9 (22.6) |
| Python 3 | 1082.3 (1035.2) | 987.1 (979.8) | 431.1 (422.6) | 535.7 (522.9) |
| Node.js | 241.4 (236.9) | 165.3 (141.4) | 231.5 (223.9) | 241.2 (236.5) |

The same numbers as a ratio to the C time (median / C median). Below 1.0 means faster
than C:

| language | fib | loop_sum | string_build | vec_memory |
| --- | ---: | ---: | ---: | ---: |
| lsaca | 1.00x | 1.05x | 14.39x | 4.68x |
| Rust | 0.51x | 1.10x | 5.16x | 1.36x |
| Python 3 | 35.42x | 90.88x | 29.14x | 29.10x |
| Node.js | 7.90x | 15.22x | 15.65x | 13.10x |

### Peak memory

MiB of peak working set, separate run. Lower is better. Remember that this is total
process footprint: it includes the runtime, the interpreter/VM and its heap.

| language | fib | loop_sum | string_build | vec_memory |
| --- | ---: | ---: | ---: | ---: |
| lsaca | 4.0 | 4.0 | 10.6 | 35.1 |
| C (gcc -O2) | 4.0 | 4.0 | 7.3 | 18.8 |
| Rust (rustc -O) | 4.1 | 4.1 | 7.3 | 18.9 |
| Python 3 | 11.0 | 11.0 | 74.1 | 142.2 |
| Node.js | 43.0 | 45.7 | 89.3 | 121.4 |

For workloads with no large data structures (`fib`, `loop_sum`) every native language
sits at the 4 MiB floor of the process image. The differences appear where the data
itself is large: `string_build` allocates a ~3.9 MB string and `vec_memory` allocates
16 MB of `Point` values. The lsaca `vec_memory` row is about 1.9x the payload because the
vector is copied by value (see the analysis below); the `string_build` row is 10.6 MiB
versus 7.3 MiB for C and Rust because the last geometric growth keeps the old and the new
buffer alive at the same time.

## Analysis

### Scalar integer loops: parity with C

`loop_sum` is the cleanest comparison: 8,000,000 iterations of `sum += i` and nothing
else. lsaca takes 11.4 ms, C 10.9 ms, Rust 12.0 ms - all within a few percent of each
other. In release mode the generated C lowers integer additions to plain C arithmetic
(the wrapping semantics are expressed as explicit `uint64_t` casts, so the compiler does
not need to keep overflow checks), and GCC -O2 turns the loop into comparable machine code
to a hand-written C loop.

This is the strongest evidence that the C backend is not intrinsically slow: when the
work does not need the runtime's help, lsaca generates C that performs like C.

### Recursion-heavy calls: fib

The recorded session has lsaca at 30.6 ms, C at 30.6 ms and Rust at 15.7 ms for the same
recursive algorithm. Two things matter here, and only one of them is about lsaca:

1. **C and Rust differ by 1.95x on identical source-level algorithms.** That is a
   code-generation effect between GCC 15.2 and LLVM (via rustc 1.99.0) on a
   self-recursive function, not a language-semantics effect. Any single workload can
   show a compiler gap of this size; it should not be read as "Rust is twice as fast".
2. **The lsaca / C difference is noisier than the other workloads.** The recorded session
   shows parity (30.6 vs 30.6 ms), and an independent interleaved re-timing of the
   previous binary pair (9 runs each, alternating) measured 36.9 ms (lsaca) vs 39.0 ms
   (C) - also roughly parity. Treat the fib row as "same ballpark", not as a stable
   speed ratio. (Rust was faster in both timings, so that part is consistent.)

The constant argument n=30 does not explain the differences: probes with
`std::hint::black_box(30)` in Rust and a `volatile` n in C produced timings within a few
percent of the plain versions, so the compilers are not constant-folding the recursion. In the
generated lsaca program n is a runtime variable anyway.

### String building: 14x C, dominated by the runtime

`string_build` appends 1,000,000 small strings. lsaca needs 212.9 ms versus 14.8 ms for
C and 76.3 ms for Rust. This is not about generating slow scalar code; it is about what a
`String` is in each implementation:

- The C program allocates one buffer of the exact final size up front and writes bytes
  into it. There are almost no function calls in the hot loop.
- The Rust program pre-allocates with `String::with_capacity`, but still allocates a
  temporary `String` for every token conversion and appends it - hence about 5x C.
- At the lsaca level the program is `s.push_str(format!("{},", v))`. Each iteration
  allocates a temporary `String` for the formatted token, appends it to `s` through the
  runtime's string function (which checks the reference count for the copy-on-write path
  and grows the buffer geometrically when needed), and then releases the temporary. So
  the loop performs one allocation plus one refcount-checked append per token, and the
  underlying buffer is repeatedly reallocated and copied. The peak-memory row reflects
  the same design: the last growth step keeps the old and the new buffer alive at the
  same time (10.6 MiB total versus 7.3-7.4 MiB for C and Rust).

None of this is inherent to the language surface: a `String` API that let the caller
append into a pre-sized buffer, or that inlined the growth path, would change the
number. The measured 14.4x is a statement about the current runtime implementation.

### Vectors of struct values: 4.7x C, value-semantics copies

`vec_memory` pushes 1,000,000 two-word structs into a `Vec`. lsaca takes 86.1 ms versus
18.4 ms in C and 24.9 ms in Rust, and peaks at 35.1 MiB versus 18.8/18.9 MiB.

The lsaca program pre-allocates with `Vec::with_capacity(n)`, exactly like the C and Rust
versions, so the remaining difference is runtime behaviour rather than a missing
optimisation:

- In the generated C, every `push` is still a call to a generic runtime insertion helper
  (`ls_vec_insert_bytes`): each call allocates a temporary element, copies the value into
  it, copies the value again into the reserved slot, and frees the temporary. One million
  pushes therefore mean one million malloc/free pairs even though the buffer never grows.
- lsaca `Vec` is a value type: copying it copies the whole buffer. `Vec::with_capacity(n)`
  produces a temporary vector that is deep-copied into the binding (a second 16 MB buffer
  right after allocation), and `for p in points` deep-copies the entire vector before the
  loop starts (another 16 MB). During that iteration two full vectors are alive, which is
  why peak memory is 35.1 MiB - about 1.9x the 16 MB payload - while C and Rust iterate
  over their vectors by reference.

Those are implementation costs of the current value-semantics model, not broken code
generation; the checksum is unaffected. A runtime that moved vectors and borrowed during
iteration would narrow both the time and the memory gap.

### Compile times: the pipeline, not the parser

A cold lsaca build is 2.8-3.3 s while gcc and rustc take about 0.3 s. The difference is
mostly fixed pipeline cost: CMake configuration of the generated project and compiling
the whole C runtime into the executable, not parsing the `.ls` program itself (the C
emission step is a small part of the run; the harness does not separately record it).
A cached build is 17-20 ms because the content-addressed cache returns the previous
executable without invoking CMake or GCC at all.

This means the answer to "lsaca compile speed" depends entirely on whether the cache is
warm. A first build is slower than C or Rust by an order of magnitude; rebuilding an
*unchanged* program is faster than either, because the content-addressed cache returns
the existing executable and skips CMake and GCC entirely. The cache key covers the
generated program, so any edit to the `.ls` source misses the cache and pays the full
cold cost again - there is no incremental compile in this pipeline yet.

### Python and Node.js: start-up, interpretation and JIT

Python is 29-91x slower than C on these workloads. The widest gap is `loop_sum`
(987 ms vs 10.9 ms), where the CPython interpreter executes 8,000,000 loop iterations;
the narrowest workloads are `string_build` (29.1x) and `vec_memory` (29.1x), where the
work is dominated by list appends and one `join`/a few loops rather than by Python-level
arithmetic. `fib` (35x) is a pure-call workload and shows the interpreter's call overhead
clearly.

Node.js lands between: 7.9-15.7x C. V8's JIT does well on the simple loops
(`loop_sum` 165.3 ms, 15.2x, with a fast minimum of 141.4 ms) but the numbers include V8
start-up on every run (each measured run is a fresh process), and its memory floor is
visible in the memory table (43-46 MiB for the two scalar workloads versus 4 MiB for
native code).

Interpreted-language numbers in this table are one-shot script invocations, not a warm
long-running server; that is the fair comparison for "run a small program once".

## Fairness notes and caveats

1. **Idiomatic, not identical, programs.** The suite measures each language as its users
   would write it, and that includes representation choices:

   | workload | lsaca | C | Rust | Python | Node.js |
   | --- | --- | --- | --- | --- | --- |
   | `string_build` | `String::new()` + `push_str(format!(...))` | one `malloc`ed char buffer of exact size | `String::with_capacity` + `to_string` + `push` | list of `str` + `",".join` | array + `join` |
   | `vec_memory` | `Vec<Point>` + `with_capacity` + `push` | `malloc`ed array of structs | `Vec<Point>` with `with_capacity` | list of tuples | array of objects |

   These are all reasonable code, but they do different amounts of low-level work, and
   that is exactly what the benchmarks are measuring.

2. **Contiguous values versus per-element objects.** C, Rust and lsaca store `Point`
   values contiguously in the vector/array; Python tuples and JS objects are individual
   heap objects referenced from the list/array. The memory rows for `vec_memory` are
   largely a consequence of that representation difference, not only of the runtime.

3. **Value semantics: copies, not references.** An lsaca `Vec` is a value type. Copying
   it (assignment, passing it by value, or iterating with `for p in vec`) deep-copies the
   whole buffer, unlike Rust's move/borrow model or C's explicit pointers. `vec_memory`
   pays for this twice: the `Vec::with_capacity` temporary is copied into the binding, and
   the `for` loop copies the vector once more before iterating. This is a deliberate
   current design (the language has no references yet), and it is the main reason for the
   35.1 MiB peak versus 18.8 MiB for C; it is not a code-generation bug.

4. **lsaca compiler limitations at measurement time.** The language has no generics and
   no `Vec` sort method, so the workloads stay deliberately simple. `Vec::with_capacity` is
   supported and used by `vec_memory`; the other vector operations in the suite are
   ordinary `push`/`len`/index operations. These numbers describe one young compiler
   version (1.1.0), not the language's potential.

5. **What the lsaca release build does.** Release builds wrap `+ - *` (the generated C
   uses explicit unsigned casts), so there are no overflow checks on those operations.
   Division and remainder always go through the runtime's checked helpers because
   division by zero panics even in release; `string_build` performs one million
   `i % 1000` operations and therefore one million checked-remainder calls. Vector
   indexing is bounds-checked, but the hot loops in this suite iterate rather than index.
   The runtime's reference-count and copy-on-write checks are per string/vector call,
   while C and Rust inline their growth logic.

6. **Compiler and flags.** gcc 15.2.0 (MSYS2 UCRT64) and rustc 1.99.0, each with the
   single normal optimisation flag (`-O2` / `-O`). No LTO, PGO, `-march=native` or
   target-cpu tuning for any language. The comparison is of standard builds on one
   Windows machine, not of tuned builds on Linux.

7. **Noise and statistics.** 7 measured runs, median (and minimum) reported. The spread
   between runs is normally a few percent, but this is a laptop with P/E cores, power
   management and background activity, not a pinned benchmark server. Treat single-digit
   percentage differences as noise; the min column is shown because it is less sensitive
   to scheduling hiccups. The largest spread in the recorded session was one Node.js
   `string_build` run at 1.42x its median; most rows stayed within 10-20% of their median.
   The fib row is discussed separately above and is the least reproducible row.

8. **Peak memory is working set, not allocation.** It includes the executable, runtime
   metadata and allocator arena. For Node.js the floor is 43-46 MiB and for Python ~11 MiB,
   so their entries measure the interpreter as much as the data. Sampling can miss very
   short spikes.

9. **Start-up is included in every runtime number.** Each measured run is a new process:
   dynamic loading, runtime initialisation and (for Python/Node.js) interpreter/VM start-up
   are all part of the time. For a 12 ms workload like `loop_sum` in C, start-up is a
   noticeable fraction of the number.

10. **Compile-time columns answer different questions.** Cold lsaca includes CMake configure
   and a two-file C build; cached lsaca is a cache hit; gcc/rustc are bare single-file
   compiles. Do not compare cross-column compile numbers as if they were the same pipeline.

11. **Official build mode versus fallback.** The numbers on this page use the official
    `lsaca build -r` path (CMake + C compiler). In environments where CMake cannot be used
    (or the sandbox blocks its child processes), the harness can fall back to
    `lsaca emit-c` plus a direct `gcc -O2`, and marks those results as fallback in
    `latest.json`. Fallback numbers are **not** comparable to these and are never mixed
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
- A C compiler and CMake for the official lsaca build path. On Windows the harness finds
  gcc and CMake via `PATH` and the usual MSYS2/CMake install locations; Ninja is used if
  present; otherwise the compiler selects MinGW Makefiles on Windows or Unix Makefiles on
  POSIX (generator selection is done by the lsaca compiler itself).
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
python scripts/bench.py --programs fib,vec_memory  # only some workloads
python scripts/bench.py --languages lsaca,c        # only some languages
python scripts/bench.py --list                     # list workload/language names
python scripts/bench.py --build-mode fallback      # emit-c + gcc instead of CMake (labelled)
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
