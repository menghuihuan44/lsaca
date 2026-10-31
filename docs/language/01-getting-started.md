# Getting started

This page takes you from an empty directory to a running lsaca program, then
explains the command-line interface, the generated C project, build profiles and
diagnostics. The language itself starts in
[Program structure](02-program-structure.md).

## Requirements

To build the compiler you need:

* Rust 1.99 or newer (stable) and Cargo.
* No third-party crates: the workspace is dependency-free.

To compile lsaca programs to native executables you also need:

* A C17 compiler: GCC or Clang. On Windows the MSYS2 UCRT64 GCC toolchain is the
  supported configuration.
* A C standard library with the usual `libm` math functions (on Unix-like
  systems the emitted project links `m` automatically).

CMake is **not** required: since version 1.2.0 the compiler invokes the C
compiler directly by default. CMake 3.16 or newer (plus Ninja or MinGW
Makefiles) is only needed if you pass the global `--cmake` flag to build
through the generated CMake project instead.

## Build and install

From the repository root:

```console
cargo build --release
./target/release/lsaca version
```

Expected output:

```text
lsaca 1.6.0
```

The compiler is a single binary at `target/release/lsaca` (`lsaca.exe` on
Windows). Copy it anywhere on your `PATH`; it finds the C compiler through
ordinary environment variables (`CC`, or `LSACA_CC` to override). CMake is only
looked up when `--cmake` is used.

## Your first program

Create `hello.ls`:

```ls
fn main() {
    println!("hello, world!");
}
```

Output:
```text
hello, world!
```

Compile and run it with a single command:

```console
lsaca run hello.ls
```

`lsaca run` type-checks the program, generates C, builds an executable in the
build cache and runs it. The program's exit status becomes the command's exit
status (a panic therefore shows up as exit code 101; see
[Exit codes](#exit-codes) below).

To produce a standalone executable next to your source, use `lsaca build`:

```console
lsaca build hello.ls            # writes ./hello (hello.exe on Windows)
lsaca build hello.ls -r         # optimized release build
```

If you want to see the generated C instead of a binary, use `lsaca emit-c`. It
writes a complete C project that you can inspect, edit and build yourself:

```console
lsaca emit-c hello.ls -o hello-c
```

The directory then contains `main.c`, the runtime sources `lsaca_rt.c`,
`lsaca_rt_string.c`, `lsaca_rt_vec.c`, `lsaca_rt_fmt.c`, `lsaca_rt_map.c` and
`lsaca_rt_unicode.c`, the headers `lsaca_rt.h`, `lsaca_rt_internal.h` and
`lsaca_rt_unicode.h`, and a `CMakeLists.txt`. Very large programs are split
across several generated translation units (see
[The generated C project](#the-generated-c-project)); a small program like
`hello.ls` stays in a single `main.c`. The
project is standalone: `CMakeLists.txt` lists every generated translation unit
together with the six runtime sources, so a
plain
`cmake -S hello-c -B hello-c/build && cmake --build hello-c/build` builds it
without lsaca.

## A slightly larger tour

The following program uses a struct, an `impl` block, a `Box<T>`, an `Rc<T>`
cell, the `?` operator and a format specifier, all verified to work in 1.6.0:

```ls
struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn new(x: i64, y: i64) -> Point {
        Point { x, y }
    }

    fn manhattan(&self) -> i64 {
        self.x + self.y
    }
}

enum Shape {
    Circle(f64),
    Square(f64),
}

fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Circle(r) => 3.0 * r * r,
        Shape::Square(s) => s * s,
    }
}

fn half(n: i64) -> Option<i64> {
    if n % 2 == 0 {
        Some(n / 2)
    } else {
        None
    }
}

fn quarter(n: i64) -> Option<i64> {
    let h = half(n)?;
    Some(h / 2)
}

fn main() {
    let p = Point::new(3, 4);
    println!("manhattan = {}", p.manhattan());

    let boxed = Box::new(String::from("on the heap"));
    println!("{}", boxed.get());

    let counter = Rc::new(0);
    let shared = counter.clone();
    shared.set(counter.get() + 1);
    println!("counter = {}", counter.get());

    println!("quarter = {:?}", quarter(12));

    println!("area = {:.2}", area(Shape::Circle(2.0)));
    println!("[{:>10}]", "right");
}
```

Output:
```text
manhattan = 7
on the heap
counter = 1
quarter = Some(3)
area = 12.00
[     right]
```

The tour already uses the ownership model added in 1.4: `Box`, `Rc`, `String`
and `Vec` are move-only, so `let shared = counter.clone()` keeps the counter usable and
`boxed.get()` reads an owned copy of the payload. Binding or passing one of those
values by value moves it; `.clone()` makes an independent second value. See
[Ownership and moves](11-ownership-and-moves.md).

## How compilation works

The compiler is a classic front end followed by C code generation:

```text
source .ls
   |
   v
lexer -> parser (AST) -> name resolution + type checker -> typed IR
   |
   v
C17 code generator (main.c, split into lsaca_unit_N.c for very large programs)
   |
   v
generated C project:
    main.c (+ lsaca_unit_1.c ... lsaca_unit_N.c for very large programs)
    + lsaca_rt.c + lsaca_rt_string.c + lsaca_rt_vec.c + lsaca_rt_fmt.c
    + lsaca_rt_map.c + lsaca_rt_unicode.c + lsaca_rt.h + lsaca_rt_internal.h
    + lsaca_rt_unicode.h (+ CMakeLists.txt)
   |
   +--> direct C compiler (default) --+
   |                                  |
   +--> CMake + C compiler (--cmake) -+
                                      |
                                      v
                              native executable
```

The type checker rejects the program before any C is generated, so a program
either type-checks completely or not at all. Diagnostics are reported on the
source file with line and column, in a rustc-like layout (see
[Diagnostics](#diagnostics) below). The C runtime implements the standard
library, reference counting, the cycle collector, string/vector storage and the
panic machinery.

The **direct backend** is used by `run` and `build` by default: the compiler
writes the C sources into the build cache, invokes the C compiler for every
generated program unit and the runtime units, and links the executable itself. The generated
CMake project is still written by `emit-c`, and asking for `--cmake` makes
`run`/`build` configure and build it with CMake instead. The emitted C and the
runtime are the same either way, so the backends differ only in who drives the
compiler.

## The generated C project

`lsaca emit-c` writes plain, readable C17. The files are:

| File | Contents |
| --- | --- |
| `main.c` | Generated C for the program: one C function per lsaca `fn`, struct/enum definitions, formatting and copy/equality helpers specialized for the program's types. For a program whose emitted user code exceeds 256 KiB, most functions move to `lsaca_unit_1.c` … `lsaca_unit_N.c` (at most eight deterministic units) and `main.c` keeps the declarations and the entry point. |
| `lsaca_unit_N.c` | Additional generated translation units for very large programs; absent when the program fits in one unit. |
| `lsaca_rt.h` | Public declarations of the runtime: strings, vectors, reference counting, panic helpers, math wrappers, I/O. |
| `lsaca_rt_internal.h` | Internal declarations shared by the runtime translation units; not meant for user code. |
| `lsaca_rt.c` | Core runtime implementation: allocation and reference counting, the cycle collector, panics, and other primitives. |
| `lsaca_rt_string.c` | `String` storage, copy-on-write buffers and the string methods. |
| `lsaca_rt_vec.c` | `Vec<T>` storage, growth and element helpers. |
| `lsaca_rt_fmt.c` | Formatting machinery: `Display`/`Debug` helpers used by `print`/`println`/`format!`. |
| `lsaca_rt_map.c` | `HashMap<K, V>`: the Swiss-table layout, hashing and key helpers. |
| `lsaca_rt_unicode.c` | Unicode tables and helpers behind the character-aware `String` methods and the Rust-style `Debug` escaping; generated by `scripts/gen_unicode_tables.py`. |
| `lsaca_rt_unicode.h` | Declarations for the generated Unicode tables. |
| `CMakeLists.txt` | A complete CMake project that builds the executable (every generated translation unit plus the six runtime sources) and configures Debug/Release profiles. |

The generated project is standalone: copy the directory to another machine with
a C17 compiler (and CMake if you use the CMake project) and it builds without
lsaca. The project name in `CMakeLists.txt` is derived from the source file name.

### Build profiles

Both backends compile the same C with the same profile defines:

| Profile | Compiler flags | lsaca behavior |
| --- | --- | --- |
| Debug (default) | `-g -O0 -DLSACA_DEBUG` | Integer overflow, division by zero, shift overflow, out-of-bounds indexing and `unwrap` failures are checked and panic with a source location. |
| Release | `-O2 -DNDEBUG` | Integer arithmetic wraps (two's complement), matching Rust's release mode; out-of-bounds and `unwrap` checks remain. |

`lsaca run` and `lsaca build` use the Debug profile unless you pass
`-r`/`--release`. The direct backend applies these flags to its compiler
invocations; the emitted `CMakeLists.txt` applies them through the CMake build
type.

Leak tracking is compiled in with the rest of the Debug checks
(`-DLSACA_DEBUG`), so a normal Debug build reports leaked allocations when the
program shuts down. The `LSACA_DEBUG_RC=1` environment variable is read by the
*compiler* at build time, not by the program: it adds `-DLSACA_DEBUG_RC` to the
C compilation, which enables the same tracking in every profile - including
Release builds (`LSACA_DEBUG_RC=1 lsaca build -r hello.ls`) - and changes the
build-cache key, so the program is rebuilt. When you build an emitted project
through CMake you can also pass `-DLSACA_DEBUG_RC=ON` at configure time (see
[Memory model](05-memory-model.md)).

The same generated C compiles with `-DLSACA_DEBUG` (checked) or without it
(wrapping arithmetic); there is no separate code path per profile, only the
runtime defines.

## Command-line interface

`lsaca` takes exactly one command and an optional `.ls` file:

| Command | Description |
| --- | --- |
| `lsaca run <file.ls> [-- args...]` | Compile, build (using the build cache) and run the program. Arguments after `--` are passed to the program and visible through `args()`. |
| `lsaca build <file.ls>` | Compile and build a native executable in the output directory (default: next to the source). |
| `lsaca emit-c <file.ls>` | Write the generated C project only; do not compile or link it. |
| `lsaca check <file.ls>` | Run the front end only: lexing, parsing, name resolution and type checking. Fast; useful in editors and CI. |
| `lsaca version` | Print the compiler version. |
| `lsaca help` | Print the help text. |

Options:

| Option | Meaning |
| --- | --- |
| `-r`, `--release` | Build with optimizations and wrapping integer arithmetic. |
| `-o`, `--out-dir <DIR>` | Output directory for `build` and `emit-c`. |
| `-v`, `--verbose` | Print the commands executed by the build (the C compiler invocations, or CMake when `--cmake` is used). |
| `--keep` | Print the temporary build-tree location instead of deleting it. |
| `--cmake` | Build through the generated CMake project instead of the built-in direct C compiler backend. Requires CMake 3.16+ and a generator (Ninja or MinGW Makefiles). |
| `-h`, `--help` | Print the help text. |

The complete help text of version 1.6.0:

```console
lsaca 1.6.0 - a small statically typed language with an ORC memory model

USAGE:
    lsaca <COMMAND> [OPTIONS]

COMMANDS:
    run <file.ls> [-- args...]   compile, build (cached) and run the program
    build <file.ls>              compile and build a native executable
    emit-c <file.ls>             write the generated C project only
    check <file.ls>              run the front end and report diagnostics
    version                      print the compiler version
    help                         print this help

OPTIONS:
    -r, --release                build with optimizations and wrapping arithmetic
    -o, --out-dir <DIR>          output directory for 'build' and 'emit-c'
    -v, --verbose                print the commands executed by the build
        --keep                   print the temporary build tree location
        --cmake                  build via the generated CMake project instead
                                 of the built-in direct C compiler back end
    -h, --help                   print this help

EXIT CODES:
    0   success
    1   compile or build error
    2   command line usage error
    101 a panic inside the compiled program (propagated from the child)
```

Running `lsaca` with no arguments prints the same help and exits with code
`2`, because a command is required. An unknown command prints a short error to
stderr, suggests `lsaca help`, and also exits with `2`. A flag that only
applies to the build commands is accepted only by the commands that build:
`--cmake` is valid for `run` and `build`; `lsaca check --cmake hello.ls`
and `lsaca emit-c --cmake hello.ls` fail with
`lsaca: unknown option '--cmake' for 'lsaca check'` (respectively
`'--cmake' for 'lsaca emit-c'`) and exit with `2`. `version` and `help`
take no arguments at all; `lsaca version --cmake` and `lsaca help --cmake`
fail with `lsaca: unexpected argument '--cmake' for 'lsaca version'`
(respectively `'--cmake' for 'lsaca help'`) and exit with `2`.

## Exit codes

The CLI and the compiled programs share a small, predictable set of exit codes:

| Code | Produced by | Meaning |
| --- | --- | --- |
| `0` | CLI / program | Success. |
| `1` | CLI | A compile or build error (diagnostics were printed). |
| `2` | CLI | Command line usage error: missing command, unknown command, missing file argument, unknown option. |
| `101` | program | The program panicked. `lsaca run` propagates the child's status, so the CLI also exits 101. |
| other | program | Whatever the program passes to `exit(n)`, or returns on a normal path. |

A program that reaches the end of `main` exits `0`. Calling `exit(0)` exits
immediately with `0`; calling `exit(7)` exits with `7` and skips the rest of
the program, including reference-count releases and the leak report (see
[Memory model](05-memory-model.md)).

## Diagnostics

Front-end errors are printed to stderr in a rustc-like format: a severity and
error code, a message, the source line with a `-->` location and a `^^^^`
underline, optional notes, and a final summary line naming the file. A program
with errors is never compiled. For example:

```ls,compile-fail
fn main() {
    let answer: i64 = "forty-two";
    println!("{}", answer);
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-03.ls:2:23
   |
 2 |     let answer: i64 = "forty-two";
   |                       ^^^^^^^^^^^
   = note: expected i64, found String

lsaca: example-03.ls: could not compile example-03.ls due to previous error(s)
```

`lsaca check` exits `1` when there are errors and prints
`lsaca: <file>: ok` to stderr when the file is clean. Warnings (for example
unreachable match arms) do not change the exit code. Warnings and errors can
both be produced in one run; the summary line is printed once at the end.

## Build cache and environment

`lsaca run` and `lsaca build` cache generated C, executable and compiled
runtime objects by a content hash of the source, the compiler version and the
build mode. The cache lives, in order of preference:

1. `LSACA_CACHE_DIR`, when set (created if needed);
2. the system temporary directory, when that path is ASCII-only and writable;
3. `.lsaca-build-cache` in the current working directory.

Within the cache, each compilation gets a directory named after its content
hash (holding the generated C sources and the build tree), and the compiled C
runtime objects are shared under `_runtime/<hash>/`. The runtime is compiled
once per profile and then reused, so only the new program's generated unit or
units have to be compiled. `-v` prints every C compiler invocation verbatim, including the flags
for the chosen profile.

On this project's development machine, a cold release build of a small program
like `hello.ls` takes roughly 1-2 s and a cached rebuild a few tens of
milliseconds; timings vary with the C compiler and the machine. See
[Benchmarks](21-benchmarks.md) for the measured numbers.

Environment variables understood by the compiler and the generated runtime:

| Variable | Effect |
| --- | --- |
| `LSACA_CC` | C compiler command used for `run`/`build` (overrides `CC`). |
| `LSACA_JOBS` | Parallel build jobs for the C compiler (or for CMake when `--cmake` is used). |
| `LSACA_CACHE_DIR` | Overrides the build cache location. |
| `LSACA_DEBUG_RC` | Read by the compiler: when set to `1`, the runtime is compiled with `-DLSACA_DEBUG_RC`, so reference-counted allocations are tracked and a leak report is printed at shutdown in every profile, including Release. Emitted CMake projects expose the same switch as `-DLSACA_DEBUG_RC=ON`. |

## Troubleshooting

* **`lsaca: cannot read <file>: ... (os error 2)`** — the source path does not
  exist or cannot be read. Check the path quoting; the message after the colon
  is the operating system's localized text.
* **No C compiler found or the C compiler fails** — make sure a C17 compiler
  (`gcc` or `clang`) is on `PATH`; set `LSACA_CC` to pick a specific compiler.
  Run with `-v` to see the exact commands.
* **CMake errors** — CMake is only needed when you pass `--cmake`. Either drop
  the flag (the default backend needs no CMake) or install CMake 3.16+ with a
  supported generator, such as Ninja or MinGW Makefiles. Run with `-v` to see
  the CMake commands.
* **The program panics with `attempt to add with overflow`** — the Debug
  profile checks integer overflow. Either the arithmetic is genuinely wrong or
  you want release semantics; pass `-r` for wrapping arithmetic.
* **The program panics with `index out of bounds`** — indexing is always
  bounds-checked, in Debug and Release. Check the index; `get()` returns
  `Option<T>` instead of panicking.
* **A stale build after editing the compiler** — the cache key includes the
  compiler version/content; if in doubt delete `LSACA_CACHE_DIR` (or
  `.lsaca-build-cache`) contents.
* **Leak report at exit** — only printed when leak tracking is enabled; a
  non-zero report for a program that neither panicked nor called `exit` is a
  compiler/runtime bug, not a language feature. See
  [Memory model](05-memory-model.md).

## What next

* Learn the shape of a program: [Program structure](02-program-structure.md).
* See every literal and token: [Lexical structure](03-lexical-structure.md).
* Borrow values without giving up ownership: [References and
  borrowing](12-references-and-borrowing.md).
* Understand the memory model before writing large programs:
  [Memory model: ORC](05-memory-model.md).
* Browse the built-in API: [Standard library](15-standard-library.md).
* Explore the 1.6 features: [Characters and
  Unicode](22-characters-and-unicode.md), [Generics](17-generics.md),
  [Traits](18-traits.md), [Iterators](19-iterators.md) and
  [HashMap](20-hashmap.md).
* Compare with Rust and see the roadmap: [Roadmap](16-roadmap.md).
