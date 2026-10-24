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
* CMake 3.16 or newer (and, optionally, Ninja for faster builds).
* A C standard library with the usual `libm` math functions (on Unix-like
  systems the emitted project links `m` automatically).

## Build and install

From the repository root:

```console
cargo build --release
./target/release/lsaca version
```

Expected output:

```text
lsaca 1.1.0
```

The compiler is a single binary at `target/release/lsaca` (`lsaca.exe` on
Windows). Copy it anywhere on your `PATH`; it finds the C toolchain through
ordinary environment variables (`CC`, or `LSACA_CC` to override) and CMake
through `PATH`.

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

The directory then contains `main.c`, `lsaca_rt.h`, `lsaca_rt.c` and
`CMakeLists.txt`.

## A slightly larger tour

The following program uses a struct, an `impl` block, a `Box<T>`, an `Rc<T>`
cell and a format specifier, all verified to work in 1.1.0:

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

fn main() {
    let p = Point::new(3, 4);
    println!("manhattan = {}", p.manhattan());

    let boxed = Box::new(String::from("on the heap"));
    println!("{}", boxed.get());

    let counter = Rc::new(0);
    let shared = counter;
    shared.set(counter.get() + 1);
    println!("counter = {}", counter.get());

    println!("area = {:.2}", area(Shape::Circle(2.0)));
    println!("[{:>10}]", "right");
}
```

Output:
```text
manhattan = 7
on the heap
counter = 1
area = 12.00
[     right]
```

## How compilation works

The compiler is a classic front end followed by C code generation:

```text
source .ls
   |
   v
lexer -> parser (AST) -> name resolution + type checker -> typed IR
   |
   v
C17 code generator (main.c)
   |
   v
generated C project: main.c + lsaca_rt.h + lsaca_rt.c + CMakeLists.txt
   |
   v
CMake + C compiler -> native executable
```

The type checker rejects the program before any C is generated, so a program
either type-checks completely or not at all. Diagnostics are reported on the
source file with line and column, in a rustc-like layout (see
[Diagnostics](#diagnostics) below). The C runtime implements the standard
library, reference counting, the cycle collector and the panic machinery.

## The generated C project

`lsaca emit-c` writes plain, readable C17. The files are:

| File | Contents |
| --- | --- |
| `main.c` | Generated C for the program: one C function per lsaca `fn`, struct/enum definitions, formatting and copy/equality helpers specialized for the program's types. |
| `lsaca_rt.h` | Public declarations of the runtime: strings, vectors, reference counting, panic helpers, math wrappers, I/O. |
| `lsaca_rt.c` | The runtime implementation, compiled together with the generated code. |
| `CMakeLists.txt` | A complete CMake project that builds the executable and configures Debug/Release profiles. |

The generated project is standalone: copy the directory to another machine with
a C17 compiler and CMake and it builds without lsaca. The project name in
`CMakeLists.txt` is derived from the source file name.

### Build profiles

The generated `CMakeLists.txt` selects behavior with the CMake build type:

| Profile | Compiler flags | lsaca behavior |
| --- | --- | --- |
| Debug (default) | `-g -O0 -DLSACA_DEBUG` | Integer overflow, division by zero, shift overflow, out-of-bounds indexing and `unwrap` failures are checked and panic with a source location. |
| Release | `-O2 -DNDEBUG` | Integer arithmetic wraps (two's complement), matching Rust's release mode; out-of-bounds and `unwrap` checks remain. |

`lsaca run` and `lsaca build` use the Debug profile unless you pass
`-r`/`--release`. When building by hand you can also pass
`-DLSACA_DEBUG_RC=ON` to CMake to enable leak tracking in a Release build (see
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
| `lsaca emit-c <file.ls>` | Write the generated C project only; do not invoke CMake or the C compiler. |
| `lsaca check <file.ls>` | Run the front end only: lexing, parsing, name resolution and type checking. Fast; useful in editors and CI. |
| `lsaca version` | Print the compiler version. |
| `lsaca help` | Print the help text. |

Options:

| Option | Meaning |
| --- | --- |
| `-r`, `--release` | Build with optimizations and wrapping integer arithmetic. |
| `-o`, `--out-dir <DIR>` | Output directory for `build` and `emit-c`. |
| `-v`, `--verbose` | Print the commands executed by the build (CMake and the C compiler). |
| `--keep` | Print the temporary build-tree location instead of deleting it. |
| `-h`, `--help` | Print the help text. |

The complete help text of version 1.1.0:

```console
lsaca 1.1.0 - a small statically typed language with an ORC memory model

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
    -h, --help                   print this help

EXIT CODES:
    0   success
    1   compile or build error
    2   command line usage error
    101 a panic inside the compiled program (propagated from the child)
```

Running `lsaca` with no arguments prints the same help and exits with code
`2`, because a command is required. An unknown command prints a short error to
stderr, suggests `lsaca help`, and also exits with `2`.

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

`lsaca run` and `lsaca build` cache generated C and built objects by a content
hash of the source and compiler version. The cache lives, in order of
preference:

1. `LSACA_CACHE_DIR`, when set (created if needed);
2. the system temporary directory, when that path is ASCII-only and writable;
3. `.lsaca-build-cache` next to the source file.

Environment variables understood by the compiler and the generated runtime:

| Variable | Effect |
| --- | --- |
| `LSACA_CC` | C compiler command used for `run`/`build` (overrides `CC`). |
| `LSACA_JOBS` | Parallel build jobs passed to CMake. |
| `LSACA_CACHE_DIR` | Overrides the build cache location. |
| `LSACA_DEBUG_RC` | When set to `1` in the environment of a Debug-built program (or with `-DLSACA_DEBUG_RC=ON` in Release), the runtime tracks reference-counted allocations and prints a leak report at shutdown. |

## Troubleshooting

* **`lsaca: cannot read <file>: ... (os error 2)`** — the source path does not
  exist or cannot be read. Check the path quoting; the message after the colon
  is the operating system's localized text.
* **CMake not found or the C compiler fails** — make sure a C17 compiler and
  CMake 3.16+ are on `PATH`; set `LSACA_CC` to pick a specific compiler. Run
  with `-v` to see the exact commands.
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
* Understand the memory model before writing large programs:
  [Memory model: ORC](05-memory-model.md).
* Browse the built-in API: [Standard library](13-standard-library.md).
* Compare with Rust and see the roadmap: [Roadmap](14-roadmap.md).
