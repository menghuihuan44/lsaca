# lsaca

**lsaca** is an independent, from-scratch implementation of a dynamically typed,
bytecode-compiled scripting language, written in C17 with no third-party
dependencies — only the C standard library.

It is **not** an educational project and not a toy: the goal is a real,
general-purpose language that people can actually write programs in. It is,
however, still at a very early stage of development (**pre-alpha, v0.0.1**), and
this README is deliberately honest about what works, what is broken and what is
still missing. Please read [Current Status](#current-status),
[Known Limitations](#known-limitations) and [Roadmap](#roadmap) before you build
anything on top of it.

What already exists is a complete, working pipeline: a hand-written scanner, a
Pratt parser, a 32-bit fixed-width bytecode format, a register-based virtual
machine with real call frames, and a mark-sweep garbage collector. What does not
exist yet is most of what makes an interpreter a *usable language*: closures,
tables, classes, a standard library, and so on.

| | |
| --- | --- |
| Language name | lsaca |
| File extension | `.ls` |
| Implementation | C17, standard library only (no third-party code) |
| Execution model | scanner → Pratt parser → register-based bytecode VM |
| Version | 0.0.1 (pre-alpha) |
| License | [Apache License 2.0](LICENSE) |

> **These are influences, not a tutorial lineage.** lsaca's fixed-width
> `ABC`/`ABx`/`Ax` instruction encoding is inspired by
> [Lua](https://www.lua.org/)'s, and the overall pipeline follows the
> architecture described in
> [*Crafting Interpreters*](https://craftinginterpreters.com/). No source code
> was copied from either work — see [Acknowledgements](#acknowledgements).

---

## Current Status

Everything below was checked against the source in this repository and by
running the programs in [`test/`](#tests) with the current build. `⚠️` marks
features that work but have sharp edges you should know about.

| Area | Status | Evidence / notes |
| --- | --- | --- |
| Scanner: tokens, `//` and `/* */` comments, keywords, line tracking | ✅ works | `src/lsaca_scanner.c` |
| Values: `null`, `boolean`, `i64`, `f64`, `str`, `func` | ✅ works | six runtime types, `src/base/lsaca_any.h` |
| Variables: `any x = ...;`, block scoping, shadowing, chained assignment | ✅ works | `test_var1.ls` … `test_var5.ls` |
| Operators: `+ - * /`, unary `- !`, `< > <= >=`, `== !=` | ✅ works | `test_var2.ls`, `test_var6.ls`, `test_var7.ls` |
| Short-circuit `and` / `or` | ✅ works | `test_cond3.ls` |
| `if` / `else`, `while`, C-style `for`, `break` / `continue` | ✅ works | `test_cond1.ls`, `test_cond4.ls` … `test_cond6.ls` |
| User-defined functions: parameters, `return`, call frames, arity checks | ⚠️ partial | `test_func1.ls`; see [Functions](#functions) |
| Constant pool, 32-bit bytecode, disassembler | ✅ works | `ABC` / `ABx` / `Ax` encodings |
| Register VM: 256-register windows, up to 64 call frames, 22 opcodes | ✅ works | `src/lsaca_vm.c`, `src/lsaca_disasm.c` |
| Mark-sweep garbage collector | ✅ works | `src/base/lsaca_gc.c`; threshold only grows |
| GC-managed strings | ⚠️ partial | equality only — no concatenation, indexing, length or escapes |
| Dynamic array | ✅ internal | used as the chunk constant pool |
| Swiss-table hash map | ✅ internal, unused | implemented and GC-aware, unreachable from scripts |
| Classes, objects, `this`, `super` | ❌ missing | keywords are scanned, then ignored |
| Tables / arrays as language values | ❌ missing | C infrastructure exists, syntax does not |
| Globals, modules, imports, standard library | ❌ missing | |
| Automated test runner, unit tests, CI | ❌ missing | 16 hand-run `.ls` scripts |

---

## Quick Start

### Requirements

* CMake ≥ 3.20
* A C17 compiler — GCC, Clang or MSVC (the build configures a strict,
  warning-heavy flag set for each of them)
* [Ninja](https://ninja-build.org/) is optional, but it is what the examples use
* No third-party libraries

The build below was verified with CMake 4.1.0, Ninja 1.13.2 and GCC 15.2.0
(MSYS2 UCRT64) on Windows.

### Build

```bash
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

Swap the generator and compiler as needed:

```bash
# Unix Makefiles + Clang
cmake -S . -B build -G "Unix Makefiles" -DCMAKE_C_COMPILER=clang -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

`Debug` builds use `-O0 -g3` (MSVC: `/Od /Zi`); `Release` builds use `-O3`
(MSVC: `/O2`) plus link-time optimization (`-flto` / `-flto=thin` / `/GL`) and
section garbage collection (`-fdata-sections -ffunction-sections` with
`-Wl,--gc-sections`; MSVC: `/Gy /OPT:REF`). The executable is produced at
`build/lsaca` (`build/lsaca.exe` on Windows).

### Run

```bash
# Interactive REPL (one line at a time)
./build/lsaca

# Run a script file
./build/lsaca path/to/script.ls
```

* Only files whose extension is exactly `.ls` are accepted; anything else exits
  with `Error: Invalid file format.`
* **Exit codes:** `0` success, `1` compilation error, `2` runtime error.
* The REPL compiles **each line as an independent program**, so variables do not
  survive from one line to the next and multi-line constructs cannot be typed
  interactively. Use a file for anything real.

### A first program

```text
any x = "hello world!";
print x;
```

```text
hello world!
```

Two things to know before running anything:

* **`print` is a statement, not a function** — `print x;`, with a semicolon.
  It is explicitly marked `/* temp */` in the source and will be replaced by
  real builtin functions later.
* **Debug output is currently always on.** Ordinary runs write the disassembled
  bytecode and a per-instruction execution trace (including the current frame's
  constant pool) to **stdout**, and garbage-collector log lines / runtime errors
  to **stderr**. This is controlled by three macros in `src/lsaca_common.h`
  (`LS_DISASM_PRINT_CODE`, `LS_DISASM_TRACE_EXECUTION`, `LS_DEBUG_GC`) that are
  hardcoded to `1`. Exposing them as CMake options is on the
  [roadmap](#roadmap).

---

## Language Tour

Every example below is taken from (or verified against) the programs in
[`test/`](#tests), and all of them run with the current build.

### Values and types

lsaca is dynamically typed. A value is one of six runtime types:

| Type | Literal syntax | Examples |
| --- | --- | --- |
| `null` | `null` | `null` |
| `boolean` | `true` / `false` | `true` |
| `i64` | integer literal | `42`, `-7` |
| `f64` | floating-point literal | `3.14`, `1.0` |
| `str` | double-quoted string | `"hi"` |
| `func` | *(no literal — produced by a `func` declaration)* | `add2` |

There is **no implicit truthiness**: conditions must be actual booleans.
`if (1)` fails at runtime with `Condition must be a boolean expression.`, and
`and` / `or` / `!` require boolean operands.

`print` renders values as: `null`, `true` / `false`, decimal integers,
`%f`-formatted floats (so `1.0 / 3.0` prints `0.333333`), raw string contents,
and `<func name>` for functions.

### Variables and block scoping

Every variable is declared with `any`, currently the only declaration keyword:

```text
any a = 1;
any b = 2;
any c = 3;
any d = a + b * c - a;   // 6
print d;
```

A declaration without an initializer is `null`:

```text
any x;      // x == null
```

Scopes are block-based, and inner scopes may shadow outer ones
(`test_var3.ls`):

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

Assignment chains right-to-left (`test_var4.ls`):

```text
any a; any b; any c;
a = b = c = 100;
print a; print b; print c;   // 100 100 100
```

These mistakes are rejected while compiling, not at runtime:

| Program | Compile error |
| --- | --- |
| `any a = 1; any a = 99;` | `Already a variable with this name in this scope.` |
| `any f = f;` | `Cannot read local variable in its own initializer.` |
| `print zzz;` | `Undefined variable.` |
| `1 = 2;` | `Invalid assignment target.` |

### Operators

```text
any a = 5;
any b = - (a + 10) * -2;   // 30
print b;

print 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10;   // 55
```

Arithmetic is strict: both operands must be the **same** type, and that type
must be `i64` or `f64`.

```text
print 1 + 2.0;
```

```text
Both operands must be of the same type.
Left value's type is i64 and right value's type is f64.
[line 1] in script
```

Other runtime rules worth knowing:

* `1 / 0` on integers: `Division by zero is not allowed for i64 operands.`
  (float division by zero is not checked).
* Integer arithmetic wraps on overflow — it is performed in unsigned 64-bit and
  cast back — and overflow is **not** detected.
* `==` / `!=` require the same type and accept `boolean`, `i64`, `f64` and
  `str`. `null` is special and may be compared against any type:
  `null == null` → `true`, `null == 1` → `false`, `null != 1` → `true`.
* Function values cannot be compared: `f == f` is a runtime error
  (`Equality comparison requires boolean, i64, f64 or str operands.`).
* Strings are not concatenable: `"a" + "b"` fails with
  `Addition requires i64 or f64 operands.`

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

`while` (`test_cond4.ls`):

```text
any i = 0;
while (i < 3) {
    print "looping";
    i = i + 1;
}
```

C-style `for` — any of the three clauses may be omitted (`test_cond5.ls`):

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

`break` / `continue`, including in nested loops (`test_cond6.ls`):

```text
for (any i = 0; i < 5; i = i + 1) {
    if (i == 1) { continue; }    // skip 1
    if (i == 3) { break; }       // stop at 3
    print "for loop i:";
    print i;                     // 0, 2
}
```

`break` / `continue` outside a loop are compile errors. `return` is allowed at
script level and stops the whole script.

### Short-circuit `and` / `or`

The right-hand side is not evaluated once the result is known, but both operands
still have to be booleans whenever they *are* evaluated (`test_cond3.ls`):

```text
any a = "str";
if (false and a == "str") {      // right side never evaluated
    print "won't print";
}
if (true or a != "str") {        // right side never evaluated
    print "won't print";
}
```

### Functions

Functions are declared with `func`, take `any` parameters, and may declare a
return type with `-> any` (`test_func1.ls`):

```text
func add2(any a, any b) -> any {
    return a + b;
}

print add2(1, 2);   // 3
```

Rules that are enforced today:

* `any` is the **only** supported annotation for parameters and return types:
  `func f(int a)` fails with
  `Currently only 'any' parameter type is supported.`
* Without `-> any`, the function may not return a value
  (`Cannot return a value from a function without return type.`) and implicitly
  returns `null`. With `-> any`, a bare `return;` fails with
  `Must return a value in function with return type.`
* With `-> any`, the body must end with a `return` *value*, otherwise you get
  `Missing 'return' at end of function with return type.` The check looks at the
  last instruction the compiler emitted, which is why
  `if (...) { return 1; } else { return 2; }` is accepted while a body ending in
  a `while` loop is rejected even if every path returns.
* Calls check arity at runtime: `Expected 2 arguments but got 1.`; calling
  something that is not a function gives `Can only call functions.`
* At most 64 call frames may be active — the next call fails with
  `Stack overflow.`
* A function must be **declared before** the statement that uses it — there is
  no hoisting.
* `func` declarations may be nested inside a function body, and the enclosing
  body can call the nested function after its declaration.

#### Functions are first-class values

A function is a normal value of type `func`: it can be stored in a variable,
passed as an argument, and called through that value.

```text
func inc(any v) -> any { return v + 1; }
func app(any f, any n) -> any { return f(n); }

print app(inc, 41);    // 42
any g = inc;
print g(41);           // 42
```

#### Visibility — this is the big one

Function bodies do **not** form closures, and there are no globals. A function
body can only see:

1. its own parameters,
2. variables declared in its own body,
3. functions declared inside its own body (after their declaration).

It cannot see the enclosing script's variables, and it cannot see — or call —
other functions by name:

```text
any x = 7;
func bad(any a) -> any { return a + x; }
// [line 1] Error at 'x': Undefined variable.
```

```text
func even(any n) -> any { return n; }
func odd(any n) -> any { return even(n); }
// [line 1] Error at 'even': Undefined variable.
```

This also means a function cannot call itself **by name**: writing `fib(n - 1)`
inside `fib` is an `Undefined variable.` compile error.

#### Recursion today (and one real bug)

The only way to recurse is to pass the function to itself as an argument. That
works — *if* the recursive argument is a literal or an already-computed local:

```text
/* Works: the argument is a local variable. */
func count(any self, any n) -> any {
    if (n < 2) { return n; }
    any m = n - 1;
    return self(self, m);
}
print count(count, 5);   // 1
```

```text
/* Broken today: computing the argument inline clobbers the argument slot. */
func count(any self, any n) -> any {
    if (n < 2) { return n; }
    return self(self, n - 1);
}
print count(count, 5);
```

```text
Both operands must be of the same type.
Left value's type is func and right value's type is i64.
```

That is a register-allocation bug in the compiler, not a design decision, and
fixing it is the first item on the [roadmap](#roadmap).
**Workaround:** assign the argument to a local first, as in the working example
above.

---

## How It Works

lsaca is a classic three-stage bytecode interpreter:

```text
source ──scanner──▶ tokens ──compiler──▶ bytecode chunk ──VM──▶ output
```

### Frontend: scanner and compiler

**Scanner** (`src/lsaca_scanner.c`) turns source text into tokens. It tracks line
numbers for error messages, skips whitespace, supports `//` line comments and
`/* ... */` block comments, recognises all keywords, and classifies number
literals as `int64` or `float64`. Strings are scanned up to the closing `"` —
escape sequences are **not** decoded. An unterminated string produces
`Unterminated string literal.`

**Compiler** (`src/lsaca_compiler.c`) is a hand-written **Pratt parser**. Every
token type has a parse rule describing its prefix function, infix function and
precedence. Identifiers are resolved against a compile-time scope stack; each
declared local is assigned to a VM register and registers are reused when scopes
end. The compiler also performs error recovery: after a syntax error it sets
`panic_mode` and `synchronize()` skips ahead to the next likely statement
boundary.

Precedence, lowest to highest:

```text
a = b        or       and      == !=     < > <= >=      + -       * /       ! -        . ()
assignment   or       and     equality   comparison     term      factor    unary      call/primary
```

Facilities the compiler enforces at compile time (all with the exact messages
listed earlier): redeclaration in the same scope, reads of a variable inside its
own initializer, undefined variables, invalid assignment targets, `break` /
`continue` outside a loop, more than 255 arguments or parameters, too many
constants in one chunk, jump distances that exceed the 24-bit encoding, running
out of the 256 registers of a frame (`Too many registers are in use.`), and the
`-> any` return rules.

### Bytecode and chunks

A *chunk* (`src/lsaca_chunk.h`) is a constant pool (a `ls_array_t` of values)
plus a growable array of 32-bit instructions with a parallel line-number array.
Instructions use three fixed-width encodings, inspired by Lua:

```text
ABC:  [ C(8) | B(8) | A(8) | OP(8) ]
ABx:  [     Bx(16)      | A(8) | OP(8) ]
Ax:   [          Ax(24)         | OP(8) ]
```

Jump offsets carry a sign (`sBx`, `sAx`), so they can point backwards for loops.

The 22 opcodes (`src/lsaca_chunk.h`, named in `src/lsaca_disasm.c`):

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
| `CALL` | `R(A) := call R(A)` with `B` arguments in `R(A+1) … R(A+B)` |
| `RETURN` | return `R(A)` (the chunk header comment still describes `R(A) … R(B)` multi-value returns; only one value is implemented) |
| `PRINT` | `print R(A)` — *temporary* opcode |

The disassembler (`src/lsaca_disasm.c`) renders a chunk as readable assembly.
For `print 1 + 2;` the current build prints exactly this:

```text
1    0000: LOADK    R0 K0 '1'
|    0001: LOADK    R1 K1 '2'
|    0002: ADD      R0 R0 R1
|    0003: PRINT    R0 R0 R0
2    0004: RETURN   R0 R0 R0
```

(The leading number is the source line, and `|` means "same line as above".)

### Runtime: the register VM

`src/lsaca_vm.c` is a switch-based dispatch loop. There is a single global VM
instance (`ls_vm`), a fixed value stack of

```text
LS_MAX_STACK = LS_MAX_FRAMES * LS_UINT8_COUNT = 64 * 256 = 16384 values
```

and up to `LS_MAX_FRAMES` (64) call frames. Each frame owns a **256-register
window** into that stack: a frame stores the function being executed, its
instruction pointer, and a pointer to the first of its 256 registers. When
`CALL` runs, the callee's window starts at `&caller_regs[A + 1]`, so arguments
placed at `R(A+1) …` become the callee's `R0 …` — parameters are simply the first
registers of the new window. `RETURN` copies `R(A)` into the caller's `R(A)` of
the call instruction and pops the frame.

Every arithmetic and comparison opcode validates its operands first:

* `+ - * / < > <= >=` require both operands to be the same type, and that type
  must be `i64` or `f64`; mixed or non-numeric operands are runtime errors.
* `==` / `!=` accept `boolean`, `i64`, `f64` and `str`; `null` may be compared
  against any type, and everything else is rejected.
* `!` and loop/if conditions require booleans.
* Integer division by zero is rejected.

Runtime errors are reported on stderr with the offending types and a small stack
trace, innermost frame first:

```text
Both operands must be of the same type.
Left value's type is i64 and right value's type is f64.
[line 1] in bad()
[line 1] in script
```

Because debug output is on by default, one payload example is useful. For
`test/test_func1.ls`, the compiler dumps the script chunk like this:

```text
== <script> ==
3    0000: LOADK    R0 K0 '<func add2>'
5    0001: MOVE     R1 R0 R0
|    0002: LOADK    R3 K1 '1'
|    0003: MOVE     R2 R3 R0
|    0004: LOADK    R4 K2 '2'
|    0005: MOVE     R3 R4 R0
|    0006: CALL     R1 R2 R0
|    0007: PRINT    R1 R0 R0
|    0008: RETURN   R0 R0 R0
```

Reading it: `LOADK R0 K0` loads the *function object* (functions are constants),
the next five instructions copy that value into a call base at `R1` and place the
arguments `1` and `2` in `R2` and `R3`, and `CALL R1 R2` means "call the function
in `R1` with 2 arguments"; the result lands back in `R1`. Only the **script**
chunk is disassembled at compile time — a function body is a chunk of its own,
and it becomes visible only in the per-instruction trace while its frame runs.
Every traced instruction is preceded by the executing frame's whole constant
pool:

```text
          [ <func add2> ][ 1 ][ 2 ]
3    0000: LOADK    R0 K0 '<func add2>'
```

That pair is the script frame at its first instruction, whose pool holds the
function object plus the two integers. As soon as `CALL` enters `add2`, the same
two lines describe that function's chunk instead: an empty constant-pool line
(the function uses no constants) and its first instruction, at source line 2:

```text
2    0000: ADD      R2 R0 R1
```

### Values and memory (`src/base/`)

* **`lsaca_any.h`** — `ls_any_t`, a tagged union holding all six value types
  (`type` plus a union of `bool`, `int64_t`, `double`, `ls_string_t *`,
  `ls_function_t *`), with `LS_IS_*` / `LS_AS_*` accessors and `LS_*_ANY`
  constructors. `ls_any_print()` and `ls_any_type_name()` provide the rendering
  and type names used by `print` and by error messages.
* **`lsaca_gc.c`** — a **mark-sweep** collector. Every allocation is preceded by
  a header holding the next-object link, payload size, element count, a type
  descriptor, a magic word (`0xCAFEBABE`) and a mark bit. Collection starts from
  a linked list of registered *roots* (pointers-to-pointers), marks reachable
  objects, sweeps the rest, then grows the threshold
  (`total_alloc_size >= threshold` triggers a collection; if that is not enough,
  the threshold doubles). Two allocation entry points are used:
  `ls_gc_alloc()` for arrays of a known element type and `ls_gc_realloc()` for
  plain buffers and strings. Passing an interior or stale pointer to the marker
  aborts the process with a `GC FATAL ERROR` diagnostic instead of corrupting
  memory. The `LS_GC_TYPE` macro describes an object array (element size, static
  reference offsets, or a `trace` callback) so `ls_any_t` arrays, map entries and
  function objects can be traced precisely.
* **`lsaca_object.c`** — `ls_function_t`: a chunk + name + arity, traced through
  its name, code, line array and constant pool.
* **`lsaca_string.c`** — GC-managed, length-prefixed byte strings
  (`size`, `capacity`, bytes inline via a flexible array member, always
  NUL-terminated). `ls_string_destroy()` only zeroes the fields; the memory
  belongs to the GC. UTF-8 length/decode/encode helpers already exist but are not
  used yet.
* **`lsaca_array.c`** — a growable array of `ls_any_t` (doubling growth) living in
  GC memory; today it backs the chunk constant pool.
* **`lsaca_map.c`** — a **Swiss-table-style** hash map: 8-wide metadata groups,
  H2-byte filtering, tombstones instead of backward-shift deletion, and a 0.875
  load factor. It hashes strings with FNV-1a and other values with a
  splitmix-style mix. It is GC-aware and complete (insert / erase / contains /
  at) but **is not wired into the language yet**; some of its error paths are
  still marked `/* temp */` and print `Key not found in map` to stderr.

A note on collection timing: allocations happen only while *compiling* (chunks,
string literals, function objects), because the VM allocates nothing while it
executes. That also means the VM's register window is not registered as a GC
root — something that must be added as soon as runtime allocation appears.

### CLI (`src/main.c`)

`main` initialises the GC and the VM, then either enters the REPL (no arguments)
or runs a single `.ls` script. It validates the file extension, propagates the
VM status as the process exit code (`0` / `1` / `2`), and shuts the GC and VM
down on success. Anything other than `lsaca` or `lsaca <script>` prints
`Usage: lsaca [script]`.

---

## Project Layout

```text
lsaca/
├── CMakeLists.txt           Build config + strict per-compiler warning flags
├── LICENSE                  Apache License 2.0
├── README.md                This file
├── src/
│   ├── main.c               CLI entry point (REPL / single-file runner)
│   ├── lsaca_common.h       Shared includes + the three debug-output switches
│   ├── lsaca_scanner.c/.h   Tokenizer
│   ├── lsaca_compiler.c/.h  Pratt parser → bytecode emitter
│   ├── lsaca_chunk.c/.h     Chunk: constant pool + instructions + line data
│   ├── lsaca_vm.c/.h        Register-based VM, call frames, runtime errors
│   ├── lsaca_disasm.c/.h    Bytecode disassembler
│   └── base/
│       ├── lsaca_any.c/.h      Tagged-union value type (`ls_any_t`)
│       ├── lsaca_gc.c/.h       Mark-sweep garbage collector
│       ├── lsaca_object.c/.h   `ls_function_t` (chunk + name + arity)
│       ├── lsaca_string.c/.h   GC-managed strings (+ unused UTF-8 helpers)
│       ├── lsaca_array.c/.h    Growable array of `ls_any_t`
│       └── lsaca_map.c/.h      Swiss-table hash map (not wired up yet)
└── test/
    ├── hello.ls             Hello world
    ├── test_var1.ls …       Variables, scoping, expressions
    ├── test_cond1.ls …      Conditionals, loops, break/continue
    ├── test_func1.ls        A user-defined function with a return value
    └── test_gc1.ls          Allocation pressure across scopes and calls
```

---

## Tests

`test/` holds hand-written `.ls` example programs. Every one of them was run
against the current build and passes. There is **no automated test runner, no
unit-test framework and no CI** yet — you run them by hand:

```bash
for f in test/*.ls; do echo "== $f =="; ./build/lsaca "$f" || echo "FAILED"; done
```

Beware that each run also dumps bytecode, per-instruction traces and GC logs, so
the output is long; the program's own output is the `print` result lines.

| File | What it exercises |
| --- | --- |
| `hello.ls` | Strings and `print` |
| `test_var1.ls` | Nested block scoping; commented-out programs that must fail to compile (redeclaration, reading a variable in its own initializer) |
| `test_var2.ls` | Operator precedence (`a + b * c - a`) |
| `test_var3.ls` | Shadowing across nested scopes |
| `test_var4.ls` | Chained assignment (`a = b = c = 100`) and `null` defaults |
| `test_var5.ls` | Writing to a variable of an enclosing scope |
| `test_var6.ls` | Unary minus and negation (`- (a + 10) * -2`) |
| `test_var7.ls` | Left-associative summation `1 + … + 10` |
| `test_cond1.ls` | `if` / `else` |
| `test_cond2.ls` | Boolean-only conditions (documents that `if (1)` is a runtime error) |
| `test_cond3.ls` | Short-circuit `and` / `or` |
| `test_cond4.ls` | `while` |
| `test_cond5.ls` | `for`, in both the full and the abbreviated form |
| `test_cond6.ls` | `break` / `continue`, including nested loops |
| `test_func1.ls` | `func` declaration, parameters, `return`, call, `-> any` |
| `test_gc1.ls` | Many string constants plus a function, a loop and a nested scope — exercises the collector's growth path |

---

## Known Limitations

An honest list of what does not work, what is rough, and what is known to be
broken. Nothing here is hidden behind a marketing hedge.

### Language and semantics

* **No closures and no upvalues.** A function body cannot see the scope it is
  defined in, so a `func` cannot capture anything. Combined with the absence of
  globals, this makes functions close to isolated: parameters and locals only.
* **Functions cannot call each other by name** and cannot call themselves by
  name, so there is no named recursion and no mutual recursion.
* **Recursion through function values is buggy.** Passing the function to itself
  works only when the recursive argument is a literal or an already-computed
  local; `return self(self, n - 1);` fails at runtime with `Both operands must be
  of the same type.` (See [Recursion today](#recursion-today-and-one-real-bug).)
* **`-> any` is checked syntactically, not semantically.** The
  `Missing 'return'` check inspects the last emitted instruction, so a body
  ending in a loop is rejected even when every path returns.
* **`any` is the only type annotation.** There is no way to declare an `i64`
  parameter or return type.
* **Function values cannot be compared** (`f == f` is a runtime error) and cannot
  be printed as anything but `<func name>`.
* **No string operations at all.** No concatenation, no length, no indexing, no
  slicing, no comparison other than `==` / `!=`, and no escape sequences in
  literals (a literal is taken verbatim between the quotes, and a string may even
  span lines).
* **No tables, arrays, maps or any other data structure in the language.** The
  `base/` array and hash map exist only as C infrastructure. There is also no
  array/map literal syntax and no indexing operator.
* **No classes, objects, methods, `this` or `super`.** Those keywords and the
  `.` / `:` tokens are scanned but no code is generated for them, so using them
  silently does something else or fails with a plain syntax error.
* **No globals**, no modules, no imports, no standard library, no builtin
  functions: `print` is a temporary statement, and there is no way to read input,
  access files, or format values.
* **No compound assignment (`+=`), no `++` / `--`**, no `switch`, no ternary
  `?:`, no `for … in`, no `do … while`, no `goto`, no labels, no user-defined
  operators.
* **`return` at script level ends the script**, mirroring the function return
  path, which is surprising in a top-level script.
* **`break` / `continue` are allowed only inside loops**, and `for` always
  requires both semicolons: `for (x < 2;)` is a compile error
  (`Expected expression.`), and a condition-only loop must be written
  `for (; x < 2;)`.

### Numbers, printing and errors

* **Integer literals are parsed with `strtoll(..., base 0)`.** A literal such as
  `08` silently becomes `0`, `09` too, and `010` is read as *octal* `8`; overflow
  is not detected. Floating literals go through `strtod` without checking for
  errors either.
* **Integer overflow wraps silently** (arithmetic runs in unsigned 64-bit and is
  cast back).
* **`f64` values print with `%f`**, i.e. six fixed decimals (`0.333333`), not the
  shortest round-tripping representation.
* **Runtime errors identify a source line, not a column or an expression**, and
  a runtime error terminates the whole VM: there is no way to catch or continue.

### Engineering and tooling

* **No automated tests, no unit tests and no CI.** The 16 `.ls` scripts are run by
  hand, and nothing checks their expected output.
* **Debug output is always on** and very loud: the trace prints the current
  frame's entire constant pool before *every* instruction. The three switches in
  `src/lsaca_common.h` are hardcoded to `1`; they are not CMake options yet.
* **The REPL is minimal.** It reads one line at a time, compiles each line as a
  separate program (no state survives, no multi-line input), and re-prints the
  whole bytecode dump for every line.
* **Single global VM and global compiler state** (`ls_vm`, `ls_parser`,
  `current`, `current_loop` are file-scope globals): the interpreter is not
  reentrant and not thread-safe, and one process runs one script.
* **No optimization whatsoever**: no constant folding, no computed-goto dispatch,
  no string interning, no constant-pool hashing, and every opcode re-checks its
  operand types at run time.
* **Fixed limits everywhere**: 64 call frames, 256 registers per frame, 255
  arguments/parameters, 65 535 constants per chunk (16-bit `Bx` index), 24-bit
  jump offsets, 1024-byte initial GC threshold. There is no stack growth and no
  register spilling.
* **The garbage collector never shrinks** its threshold, so memory use grows
  monotonically; it also runs only at allocation time, and the VM's register
  window is not registered as a root (harmless today only because the VM never
  allocates).
* **Leftovers in the code** that are documented as such: `/* temp */` markers on
  `print`, `ls_map_at`'s "print to stderr and return null" fallbacks, a few
  functions whose bodies say they do not really know what the right answer is,
  and an unused `Ax` disassembler helper that mislabels a constant index as `K`.
* **`ls_string_destroy()` does not free memory** — it only zeroes the object's
  fields, since the GC owns all string storage.
* **`LICENSE` still carries the placeholder** `Copyright [yyyy] [name of
  copyright owner]`.
* **No language specification, no changelog and no release process** beyond this
  README and the version number in `CMakeLists.txt`.

---

## Roadmap

Roughly in priority order. The first four items are the ones that most block
writing real programs; the rest is engineering and polish. Several of them are
already marked in the source with `/* temp */` or "In the future…" comments.

1. **Fix the recursive-call register bug.** `func f(any self, any n) { return
   self(self, n - 1); }` must work without the local-variable workaround. This is
   a register-allocation problem in `src/lsaca_compiler.c` (the argument slot is
   reused as a temporary), not a design decision.
2. **Make functions able to see their surroundings.** Resolve names through the
   enclosing compiler (`ls_compiler_t.enclosing` already exists) so that a
   function can call other functions, see enclosing locals (closures / upvalues)
   and refer to itself by name — which gives named recursion and mutual
   recursion.
3. **Globals.** There is no global scope at all today; a `globals` table plus
   lookup in `GET`/`SET`-style opcodes is the usual next step.
4. **String operations.** Concatenation, length, comparison, indexing and escape
   sequences in literals. This requires runtime allocation, which in turn
   requires registering the VM's register window as a GC root (see below).
5. **Classes and objects.** `class`, `this`, `super`, field/method access and
   method calls. The Swiss-table hash map in `src/base/lsaca_map.c` is already
   GC-aware and waiting for exactly this.
6. **Tables / arrays as language values.** Wire `ls_array_t` and `ls_map_t` into
   the VM, add literal syntax and an indexing operator, and let the maps hold
   arbitrary key types.
7. **Builtin functions and a standard library** (I/O, conversions, math, string
   helpers), replacing the temporary `PRINT` opcode and `print` statement with
   real calls.
8. **Modules and imports** (`import`/`require` with a file path or module table).
9. **Turn the debug switches into build options.** `LS_DISASM_PRINT_CODE`,
   `LS_DISASM_TRACE_EXECUTION` and `LS_DEBUG_GC` should be opt-in (and the
   constant-pool dump should be much quieter), so a normal run only prints the
   program's output.
10. **Harden the frontend.** Reject malformed integer literals (`08`), detect
    literal overflow, validate `strtod` results, print `f64` in the shortest
    round-tripping form, and report runtime errors with column information.
11. **Fix the known code-level defects** listed in
    [Known Limitations](#engineering-and-tooling): the
    single-value `RETURN` versus its multi-value comment, `ls_map_at`'s
    print-and-return-null fallback, and the unused/misleading `Ax` disassembler
    helper.
12. **GC work:** register the VM stack (and future runtime roots) properly, add
    shrink support so memory can be returned, and consider incremental or
    generational collection. The mark/sweep core and tracing hooks are already
    there.
13. **Testing and CI:** a runner with expected outputs for `test/*.ls`
    (CTest or a script), unit tests for the GC, array and map, plus a CI job
    building with GCC, Clang and MSVC.
14. **Performance:** computed-goto or direct-threaded dispatch, constant folding,
    constant-pool hashing, string interning, and less MOVE-heavy code generation
    (`LOADK R3` followed by `MOVE R2 R3` is everywhere).
15. **REPL and CLI polish:** multi-line input, state that survives between
    inputs, `--quiet` / `--version` flags, and a way to echo expression results.
16. **Project hygiene:** a language specification document, a changelog, release
    tags, and filling in the copyright holder in `LICENSE`.

---

## Appendix: Grammar

The grammar currently accepted by `src/lsaca_compiler.c`, in EBNF. `IDENTIFIER`
and the literals are lexical tokens; everything else is literal syntax.

```ebnf
program        → declaration* EOF

declaration    → varDecl | funcDecl | statement
varDecl        → "any" IDENTIFIER ( "=" expression )? ";"
funcDecl       → "func" IDENTIFIER "(" params? ")" ( "->" "any" )? block
params         → "any" IDENTIFIER ( "," "any" IDENTIFIER )*

statement      → printStmt | block | ifStmt | whileStmt | forStmt
               | breakStmt | continueStmt | returnStmt | exprStmt
printStmt      → "print" expression ";"
block          → "{" declaration* "}"
ifStmt         → "if" "(" expression ")" statement ( "else" statement )?
whileStmt      → "while" "(" expression ")" statement
forStmt        → "for" "(" ( varDecl | exprStmt | ";" )
                 expression? ";" expression? ")" statement
breakStmt      → "break" ";"
continueStmt   → "continue" ";"
returnStmt     → "return" expression? ";"
exprStmt       → expression ";"

expression     → assignment
assignment     → IDENTIFIER "=" assignment | logicOr
logicOr        → logicAnd ( "or" logicAnd )*
logicAnd       → equality ( "and" equality )*
equality       → comparison ( ( "==" | "!=" ) comparison )*
comparison     → term ( ( "<" | ">" | "<=" | ">=" ) term )*
term           → factor ( ( "+" | "-" ) factor )*
factor         → unary ( ( "*" | "/" ) unary )*
unary          → ( "!" | "-" ) unary | call
call           → primary ( "(" arguments? ")" )*
arguments      → expression ( "," expression )*
primary        → "null" | "true" | "false" | INT64 | FLOAT64 | STRING
               | IDENTIFIER | "(" expression ")"
```

Lexical notes:

```ebnf
comment        → "//" (any character except newline)*
               | "/*" (any character)* "*/"
INT64          → digit+
FLOAT64        → digit+ "." digit+
STRING         → '"' (any character except '"')* '"'
IDENTIFIER     → ( letter | "_" ) ( letter | digit | "_" )*
```

Details that the grammar does not show: strings may contain newlines (the line
counter follows them) and no escape sequences are decoded; `-7` is unary minus
applied to a literal, not a negative literal; assignment is right-associative
and only a bare variable name is a valid target; `else` may be attached to any
statement, not just a block.

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See `LICENSE` for the
full text. Note that its copyright line is still the upstream placeholder
(`Copyright [yyyy] [name of copyright owner]`) and still has to be filled in.

---

## Acknowledgements

Two works shaped this project's architecture and deserve explicit credit:

* **[The Lua programming language](https://www.lua.org/)** — the inspiration for
  lsaca's compact, fixed-width `ABC`/`ABx`/`Ax` instruction encoding, its
  register-window execution model and the shape of its chunk/constant-pool
  design. lsaca learns from Lua's design; it does not copy Lua's code, and it is
  not a Lua implementation.
* **[*Crafting Interpreters*](https://craftinginterpreters.com/) by Robert
  Nystrom** — the clearest practical description of the
  scanner → Pratt parser → chunked bytecode → VM pipeline that lsaca follows,
  along with its approach to type checks, stack traces and garbage collection.

lsaca is an original, from-scratch implementation. It borrows architecture and
ideas from both works, shares no source code with them, and is being developed as
a language in its own right — not as a companion project to either.
