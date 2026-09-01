# lsaca

A small, dynamically typed, bytecode-based scripting language implemented from
scratch in C11/C17. It is an educational interpreter project: a hand-written
scanner, a Pratt parser, a register-based virtual machine, and a Cheney-style
copying garbage collector — all with zero dependencies outside the C standard
library.

The design is strongly inspired by the Lua language (notably its `ABC`/`ABx`/`Ax`
fixed-width instruction encoding) and by Robert Nystrom's book
*Crafting Interpreters* (whose clox interpreter shows how every one of these
pieces fits together). See [Acknowledgements](#acknowledgements).

> **Status: pre-alpha (v0.0.1).** This is a learning project that runs real
> programs today — variables, arithmetic, `if`/`else`, `while`, `for`,
> `break`/`continue`, short-circuit `and`/`or`, and `print` — but many pieces
> of a production language are still missing or temporary. Please read
> [Current Status](#current-status) and
> [Known Limitations](#known-limitations) before you set expectations, and
> [Roadmap](#roadmap) for where things are heading.

---

## Current Status

The table below is an honest snapshot of what is implemented today. Every row
marked **done** has a working test case under [`test/`](#tests).

| Area | Status | Notes |
| --- | --- | --- |
| Lexer (tokens, comments, `//` + `/* */`) | ✅ done | Numbers, strings, identifiers, keywords |
| Primitives | ✅ done | `null`, `true`/`false`, 64-bit integers, 64-bit floats, strings |
| Variables & block scoping | ✅ done | `any x = ...;`, shadowing, chained assignment, redeclaration checks |
| Arithmetic `+ - * /` & unary `- !` | ✅ done | Strict same-type runtime checks; i64 division-by-zero guard |
| Comparison `< > <= >=` | ✅ done | i64 / f64 operands |
| Equality `== !=` | ✅ done | bool / i64 / f64 / str; `null` compares specially with any type |
| Short-circuit `and` / `or` | ✅ done | Conditions must be booleans |
| `if` / `else` | ✅ done | Boolean-only conditions enforced at runtime |
| `while` | ✅ done | Boolean-only conditions |
| C-style `for` | ✅ done | Any of the three clauses may be omitted |
| `break` / `continue` | ✅ done | Works in nested loops |
| `print` statement | ✅ done (temporary) | Explicitly marked `/* temp */` in the source; will become a builtin call |
| Constant table, 32-bit bytecode, disassembler | ✅ done | `ABC`/`ABx`/`Ax` encodings |
| Register VM (256 registers) | ✅ done | 21 opcodes |
| Cheney copying GC | ✅ done | Root tracking, automatic grow/shrink |
| Strings (heap, GC-managed) | ✅ done | But no concatenation / indexing yet |
| Dynamic array / constant buffer | ✅ done | Foundation for future data structures |
| Swiss-table hash map | ✅ done | Implemented in `src/base/`, **not yet wired into the language** |
| Functions & call frames | ❌ not yet | `return` currently ends the script |
| Classes / `this` / `super` | ❌ not yet | Keywords are recognized but not implemented |
| String operations | ❌ not yet | No `+`, no indexing, no escapes |
| Tables / arrays as language values | ❌ not yet | The `base/` implementation exists, the syntax doesn't |
| Standard library / modules / imports | ❌ not yet | |
| Global variables | ❌ not yet | Only block-scoped locals |

---

## Quick Start

### Requirements

* CMake ≥ 3.20
* A C17 compiler: GCC, Clang, or MSVC (the build sets up strict warning flags
  for each of them)
* Optionally [Ninja](https://ninja-build.org/) as the generator
* No third-party libraries — only the C standard library

### Build

```bash
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

Replace the generator/compiler as needed, e.g.:

```bash
# Unix Makefiles + Clang
cmake -S . -B build -G "Unix Makefiles" -DCMAKE_C_COMPILER=clang -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

The executable is produced at `build/lsaca` (`build/lsaca.exe` on Windows).

### Run

```bash
# Interactive REPL
./build/lsaca

# Run a script file
./build/lsaca path/to/script.ls
```

* Only files with a `.ls` extension are accepted.
* **Exit codes:** `0` success, `1` compilation error, `2` runtime error.

> **Heads-up:** debug output is currently **always on**. When you run any
> program you will also see the disassembled bytecode, a per-instruction
> execution trace, and GC statistics on stderr. This is controlled by three
> macros in `src/lsaca_common.h`
> (`LS_DISASM_PRINT_CODE`, `LS_DISASM_TRACE_EXECUTION`, `LS_DEBUG_GC`) that are
> hardcoded to `1` for now. Gating them behind the build system is on the
> [roadmap](#roadmap).

### A first program

```text
any x = "hello world!";
print x;
```

```text
hello world!
```

---

## Language Tour

The examples below are adapted from the test programs in [`test/`](#tests) and
all run correctly today.

### Data types

`lsaca` is dynamically typed. Every value is one of five runtime types:

| Type | Literal syntax | Example |
| --- | --- | --- |
| `null` | `null` | `null` |
| `boolean` | `true` / `false` | `true` |
| `i64` | integer literal | `42`, `-7` |
| `f64` | floating-point literal | `3.14`, `1.0` |
| `str` | double-quoted string | `"hi"` |

There is **no implicit truthiness**: conditions must be actual booleans.
`if (1)` is a runtime error, not "truthy", and `and`/`or` require boolean
operands.

### Variables and block scoping

Variables are declared with the `any` keyword (the only keyword allowed, for
now — the language has no static types).

```text
any a = 1;
any b = 2;
any c = 3;
any d = a + b * c - a;   // 6
print d;
```

A variable without an initializer is `null`:

```text
any x;      // x == null
```

Scoping is block-based, and inner scopes may shadow outer ones:

```text
any x = 10;
{
    any x = 20;          // shadows the outer x
    print x;             // 20
    {
        any x = 30;
        print x;         // 30
    }
    print x;             // 20
}
print x;                 // 10
```

Chained assignment works right-to-left:

```text
any a; any b; any c;
a = b = c = 100;
print a; print b; print c;   // 100 100 100
```

The compiler rejects a handful of variable bugs up front, rather than letting
them become subtle runtime bugs:

* Redeclaring a name in the **same** scope is a compile error:
  ```text
  any a = 1;
  any a = 99;   // Error: Already a variable with this name in this scope.
  ```
* Reading a variable inside its own initializer is a compile error:
  ```text
  any f = f;    // Error: Cannot read local variable in its own initializer.
  ```
* Using an undeclared variable is a compile error:
  ```text
  print zzz;    // Error: Undefined variable.
  ```

### Arithmetic and unary operators

```text
any a = 5;
any b = - (a + 10) * -2;   // 30
print b;

print 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10;   // 55
```

Operands of mixed types are rejected at runtime (`1 + 2.0` is an error), as is
integer division by zero.

### Conditionals

```text
any a = 1;
any b = 2;

if (a < b) {
    print "a is smaller";
} else {
    print "b is smaller";
}
```

### Loops

`while`:

```text
any i = 0;
while (i < 3) {
    print "looping";
    i = i + 1;
}
```

`for` — any of the three clauses may be omitted:

```text
for (any i = 0; i < 3; i = i + 1) {
    print "current i is:";
    print i;
}
print "loop finished!";

any x = 0;
for (; x < 2;) {          // no initializer, no update
    print "x is:";
    print x;
    x = x + 1;
}
```

`break` and `continue`, including in nested loops:

```text
for (any i = 0; i < 5; i = i + 1) {
    if (i == 1) { continue; }    // skip 1
    if (i == 3) { break; }       // stop at 3
    print "for loop i:";
    print i;                     // 0, 2
}
```

### Short-circuit `and` / `or`

Evaluation stops as soon as the result is known:

```text
any a = "str";
if (false and a == "str") {      // right side never evaluated
    print "won't print";
}
if (true or a != "str") {        // right side never evaluated
    print "won't print";
}
```
---

## How It Works

`lsaca` is a classic three-stage bytecode interpreter:

```
source ──scanner──▶ tokens ──compiler──▶ bytecode chunk ──VM──▶ output
```

```
src/lsaca_scanner.c      src/lsaca_compiler.c        src/lsaca_vm.c
```

### Frontend: scanner & compiler

**Scanner** (`src/lsaca_scanner.c`) turns source text into tokens. It tracks
line numbers (for error messages), skips whitespace, supports `//` line
comments and `/* ... */` block comments, recognizes all keywords, and
classifies number literals as `int64` or `float64`. Strings are scanned up to
the closing `"` — escape sequences are **not** decoded yet.

**Compiler** (`src/lsaca_compiler.c`) is a hand-written **Pratt parser**. Every
token type has a parse rule describing its prefix function, infix function, and
precedence. Identifiers are resolved against a compile-time scope stack; each
declared local is assigned a VM register, and registers are reused when scopes
end. The compiler emits instructions into a chunk and reports errors like
`Undefined variable.` / `Already a variable with this name in this scope.` /
`Cannot read local variable in its own initializer.`

Precedence, lowest to highest:

```
a = b        or       and      == !=     < > <= >=      + -       * /       ! -        . ()
assignment   or       and     equality   comparison     term      factor    unary      call/primary
```

### Bytecode & chunks

A *chunk* (`src/lsaca_chunk.h`) is a constant pool plus a growable array of
32-bit instructions and a parallel line-number array. Instructions use three
fixed-width encodings (inspired by Lua):

```
ABC:  [ C(8) | B(8) | A(8) | OP(8) ]
ABx:  [     Bx(16)      | A(8) | OP(8) ]
Ax:   [          Ax(24)         | OP(8) ]
```

Jump offsets carry a sign (`sBx`, `sAx`) so they can point backwards (loops).

**Opcodes** (21 total, in `src/lsaca_disasm.c`):

| Opcode | Meaning |
| --- | --- |
| `MOVE` | `R(A) := R(B)` |
| `LOADK` | `R(A) := K(Bx)` |
| `LOADNULL` / `LOADFALSE` / `LOADTRUE` | load a literal value |
| `NEG` / `NOT` | `R(A) := -R(B)` / `R(A) := !R(B)` |
| `BANG_EQUAL` / `EQUAL_EQUAL` / `GREATER` / `GREATER_EQUAL` / `LESS` / `LESS_EQUAL` | `R(A) := R(B) op R(C)` |
| `ADD` / `SUB` / `MUL` / `DIV` | `R(A) := R(B) op R(C)` |
| `JUMP` | `ip += sAx` |
| `JUMP_IF_FALSE` | `if R(A) == false then ip += sBx` |
| `RETURN` | `return R(A)..R(B)` — today it simply ends the program |
| `PRINT` | `print R(A)` — *temporary* opcode |

The disassembler (`src/lsaca_disasm.c`) renders a chunk as readable assembly,
e.g. for `print 1 + 2;`:

```text
== code ==
1    0000: LOADK    R0 K0 '1'
|    0001: LOADK    R1 K1 '2'
|    0002: ADD      R0 R0 R1
|    0003: PRINT    R0 R0 R0
|    0004: RETURN   R1 R0 R0
```

### Runtime: the register VM

`src/lsaca_vm.c` is a straightforward switch-based dispatch loop over a fixed
array of **256 registers** (`LS_MAX_REGS`). Every arithmetic/comparison opcode
starts by validating operand types:

* `+ - * / < > <= >=` require both operands to be the **same** type (`i64` or
  `f64`); mixed types are runtime errors.
* `==` / `!=` accept booleans, integers, floats and strings. `null` is special:
  it may be compared against any type, and `null == null` is `true`.
* Integer division by zero is rejected.
* Loops and `if` conditions must evaluate to a boolean; a non-boolean condition
  raises a runtime error such as `Condition must be a boolean expression.`

Runtime errors print the source line and the offending type(s), e.g.:

```text
Both operands must be of the same type.
Left value's type is i64 and right value's type is f64.
[line 1] in script
```

### Values, memory & base data structures (`src/base/`)

* **`lsaca_any.h`** — `ls_any_t`, a tagged union for the five value types
  (`null`, `bool`, `i64`, `f64`, `str`).
* **`lsaca_gc.c`** — a **Cheney two-space copying garbage collector**. `from`/`to`
  semi-spaces (`alloc_ptr`, `scan_ptr`), registered *roots* (stack/global C
  variables that may hold GC pointers), object headers that describe how to
  trace reference fields, and automatic growing/shrinking of the semi-spaces
  based on the live-bytes watermark.
* **`lsaca_string.c`** — GC-managed, length-prefixed byte strings
  (`size`, `capacity`, and the bytes inline via a flexible array member).
  UTF-8 helper functions already exist for future use.
* **`lsaca_array.c`** — a growable array of `ls_any_t`; currently used as the
  chunk constant pool.
* **`lsaca_map.c`** — a **Swiss-table-style hash map** (8-wide metadata groups,
  top-byte hash filtering, tombstones for deletion). It is implemented and
  GC-integrated, but has not been wired into the language yet.

### CLI (`src/main.c`)

`main` initializes the GC and VM, then either enters a REPL (no arguments) or
runs one `.ls` script. It validates the file extension, returns a non-zero exit
code on failure, and shuts the VM down cleanly on success.
---

## Project Layout

```text
lsaca/
├── CMakeLists.txt          Build config + strict per-compiler warning flags
├── LICENSE                 Apache License 2.0
├── README.md
├── src/
│   ├── main.c              CLI entry point (REPL / single-file runner)
│   ├── lsaca_common.h      Shared types + debug-output switches
│   ├── lsaca_scanner.c/.h  Tokenizer
│   ├── lsaca_compiler.c/.h Pratt parser → bytecode emitter
│   ├── lsaca_chunk.c/.h    Chunk: constant pool + instruction + line data
│   ├── lsaca_vm.c/.h       Register-based virtual machine
│   ├── lsaca_disasm.c/.h   Bytecode disassembler
│   └── base/
│       ├── lsaca_any.c/.h     Tagged union value type
│       ├── lsaca_string.c/.h  GC string
│       ├── lsaca_array.c/.h   Dynamic array of values
│       ├── lsaca_map.c/.h     Swiss-table hash map (foundation only)
│       └── lsaca_gc.c/.h      Cheney copying GC
└── test/
    ├── hello.ls            Hello world
    ├── test_cond1.ls …     Conditionals & loops (below)
    └── test_var1.ls …      Variables & expressions (below)
```

---

## Tests

The `test/` directory holds hand-written `.ls` example programs. There is no
automated test runner yet — run them manually after building:

```bash
for f in test/*.ls; do echo "== $f =="; ./build/lsaca "$f" || echo "FAILED"; done
```

| File | Exercises |
| --- | --- |
| `hello.ls` | Strings, `print` |
| `test_var1.ls` | Nested block scoping; documented (commented-out) compile-error cases |
| `test_var2.ls` | Operator precedence (`a + b * c - a`) |
| `test_var3.ls` | Shadowing across nested scopes |
| `test_var4.ls` | Chained assignment (`a = b = c = 100`) and `null` defaults |
| `test_var5.ls` | Mutating a captured variable from inner scopes |
| `test_var6.ls` | Unary minus and negation (`- (a + 10) * -2`) |
| `test_var7.ls` | Left-associative summation `1+…+10` |
| `test_cond1.ls` | `if` / `else` |
| `test_cond2.ls` | Boolean-only truthiness (documents `if (1)` is not allowed) |
| `test_cond3.ls` | Short-circuit `and` / `or` |
| `test_cond4.ls` | `while` loop |
| `test_cond5.ls` | `for` loop (both full and abbreviated forms) |
| `test_cond6.ls` | `break` / `continue`, including nested loops |
---

## Known Limitations

An honest list of what does **not** work yet or is known to be rough:

* **No functions.** `return` is emitted only at the end of a program and simply
  halts the VM; there are no user-defined functions, call frames, parameters,
  or call opcodes.
* **No classes / objects.** `class`, `this`, and `super` are recognized as
  keywords but produce no code; there is no object type.
* **No string operations.** Strings support equality but no concatenation,
  indexing, length, or escape sequences in literals.
* **No table / array syntax.** The Swiss-table map and dynamic array exist as
  C infrastructure but are not reachable from a script.
* **`print` is a temporary statement**, not a real function.
* **Debug output is always on.** Bytecode dumps, per-instruction traces, and GC
  logs are emitted during normal runs because the switches in
  `src/lsaca_common.h` are hardcoded. This makes stdout noisy and is not meant
  for end users yet.
* **Number parsing is lenient.** Integer literals are parsed with
  `strtoll(..., base 0)`, so a literal like `08` silently becomes `0` instead
  of producing an error, and overflow is not detected.
* **`f64` output is not formatted.** Floats print with a fixed `%f` (e.g.
  `0.333333`), not the shortest round-trip representation.
* **No standard library, modules, or imports.**
* **No globals** — only block-scoped local variables.
* **No automated test suite / CI.**
* **Single script + REPL only**; no nesting, importing, or multi-file support.
* **Primitive REPL.** It reads a single line at a time and compiles each line as
  an independent chunk, so there is no multi-line editing and no state carried
  between lines (each line is its own program).

---

## Roadmap

Gaps and *future work* — in rough priority order, informed by the `/* temp */`
and *"In the future…"* comments already in the code:

1. **Functions & call frames** — the biggest missing feature: real `return`
   values, parameters, and a call stack (this also requires per-function
   chunks/constant pools and call opcodes).
2. **Closures**, once functions exist, so inner scopes can capture non-locals.
3. **String concatenation and operations** (`+`, length, indexing, escapes).
4. **Classes, methods, `this` / `super`** — the scanner support is ready.
5. **Language-level tables & arrays**, wiring the existing `base/lsaca_map`
   and `lsaca_array` into the VM and giving them syntax.
6. **Global variables** and a module/import mechanism.
7. **Replace the temporary `print` statement** with real builtin functions and
   a standard library.
8. **Expose the debug switches via CMake** (`LS_DISASM_*`, `LS_DEBUG_GC`) so
   ordinary runs are quiet.
9. **Tighten number literals** — reject bad base-0 forms, detect overflow, and
   improve float formatting.
10. **Automated testing + CI** and a proper interpreter error/panic story.

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See `LICENSE` for
the full text.

---

## Acknowledgements

Two influences deserve explicit thanks:

* **[The Lua programming language](https://www.lua.org/)** (a.k.a. "standard
  Lua") — the inspiration for `lsaca`'s compact, fixed-width `ABC`/`ABx`/`Ax`
  bytecode encodings and its register-based execution model. This project aims
  to learn from, not copy, Lua's design.

* **[*Crafting Interpreters*](https://craftinginterpreters.com/) by Robert
  Nystrom** — the clearest practical guide to building a tree-walker and then a
  bytecode VM. `lsaca` follows the same learning path (scanner → Pratt parser →
  chunks with constant pools → a register VM → a copying garbage collector,
  plus careful error reporting), re-implemented from scratch in this project's
  own style and naming conventions.

`lsaca` is an original, from-scratch implementation; it borrows architecture and
ideas from the two works above but shares none of their source code.