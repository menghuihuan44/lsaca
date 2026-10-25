# lsaca C17 runtime

Portable C17 runtime for programs compiled by `lsaca`. Generated C links against
it, so the language's memory management (ORC: reference counting plus a cycle
collector), `String`, `Vec`, formatting, panics, checked integer arithmetic,
process/IO helpers and math all behave the same on every supported platform.

The runtime is written carefully against the C17 standard: no compiler-specific
extensions, no VLAs, no GNU statement expressions, no threads, no third-party
code. It compiles warning-free with
`gcc -std=c17 -Wall -Wextra -Wpedantic` and with clang.

## Files

| File | Role |
| --- | --- |
| `lsaca_rt.h` | Public ABI. The only runtime header generated code includes. Frozen function signatures and frozen layouts of `LsString`, `LsVec_String`, `LsOpt_String`, `LsRes_String_String`. |
| `lsaca_rt_internal.h` | Private contract shared by the translation units (private allocation header layout, cross-unit helpers). Generated code never includes it. |
| `lsaca_rt.c` | Core: reference counted allocator, ORC cycle collector, leak report, panics, checked integer arithmetic, process/IO, math. |
| `lsaca_rt_string.c` | `LsString` objects: construction, copy-on-write mutation, search/transform, split, parse. |
| `lsaca_rt_vec.c` | Generic vector primitives and the frozen `LsVec_String` operations. |
| `lsaca_rt_fmt.c` | Format builder, shortest-round-trip float formatting, precision/width, Debug escaping, print helpers. |
| `test/main.c` | Runtime self-test suite (`ALL CHECKS PASSED (411 checks)`), including collector, refcount and panic-path tests. |
| `test/bench_runtime.c` | Standalone micro-benchmark driver for the hot paths (not part of the smoke matrix). |
| `test/run_smoke.ps1` | Windows smoke-test driver: builds release/debug/leak-tracking configurations of the runtime and exercises the panic/leak/IO/stdin/args child-process modes. |

`crates/lsaca/src/runtime.rs` embeds every file above (except the tests) with
`include_str!` and exposes `runtime::sources() -> &'static [(name, contents)]`.
The driver writes that complete list into every generated project; the CMake
project lists every translation unit. Adding a runtime file therefore only
requires adding it to that list, not touching codegen.

## Generated project

A generated project directory contains:

    main.c                  generated program
    CMakeLists.txt          generated CMake project
    lsaca_rt.h              public runtime header
    lsaca_rt_internal.h     private header (needed by the runtime .c files)
    lsaca_rt.c
    lsaca_rt_string.c
    lsaca_rt_vec.c
    lsaca_rt_fmt.c

Build configurations:

| Configuration | Compiler flags |
| --- | --- |
| Release | `-std=c17 -O2 -DNDEBUG -Wall -Wextra` |
| Debug | `-std=c17 -O0 -g -DLSACA_DEBUG -Wall -Wextra` |
| Debug with leak tracking | additionally `-DLSACA_DEBUG_RC` (or alone) |

The runtime must be compiled with the same `-DLSACA_DEBUG`/`-DLSACA_DEBUG_RC`
choice as generated `main.c`: the leak report is part of the runtime, the
checked-arithmetic calls are emitted by codegen.

Build by hand (POSIX shell, from the generated directory):

    gcc -std=c17 -O2 -DNDEBUG -Wall -Wextra main.c \
        lsaca_rt.c lsaca_rt_string.c lsaca_rt_vec.c lsaca_rt_fmt.c -lm -o program

## Memory model (ORC)

Every managed object is a single `malloc` block: a private header (reference
count, payload size, magic word, drop/trace callbacks, collector bookkeeping)
immediately followed by the payload itself. `ls_alloc(size)` returns a zeroed
payload with reference count 1; `ls_alloc_managed(size, drop, trace)` also
registers the payload with the cycle collector. The initial reference count is
always 1, so a freshly produced value is already owned by its producer.

- `ls_retain(obj)` increments the count and returns `obj`; NULL is a no-op.
- `ls_release(obj)` decrements the count, frees at zero; NULL is a no-op.
- `ls_refcount(obj)` reports the current count (0 for NULL).
- Only cycle-capable payloads (Rc cells, closure environments) are traced; the
  collector marks a traced object as a candidate when its count drops but stays
  above zero. When enough candidates accumulate, or on shutdown, trial deletion
  finds objects only reachable from garbage cycles and frees them.
- Strings, `Vec` data buffers and other unmanaged payloads are never collector
  candidates and pay no trace bookkeeping on release.
- Releasing an invalid or already released handle prints
  `lsaca: runtime error: release of an invalid or already released reference-counted object`
  to stderr and exits with status 101. Out-of-memory is fatal in the same style.
- The runtime is not thread safe because the language has no threads.

## Frozen data representations

Generated code never redefines these; they live in `lsaca_rt.h`:

    typedef struct LsString {        /* opaque in generated code */
        size_t len;                  /* byte length (no NUL) */
        size_t cap;                  /* byte capacity of data */
        char* data;                  /* inline NUL-terminated bytes */
    } LsString;
    /* A String value is a handle: LsString* (one reference counted allocation). */

    typedef struct LsVec_String { size_t len; size_t cap; LsString** data; } LsVec_String;
    /* The vector owns one reference per element. */

    typedef struct LsOpt_String { int32_t tag; union { LsString* some; } u; } LsOpt_String;
    /* tag 0 = None, tag 1 = Some. */

    typedef struct LsRes_String_String { int32_t tag; union { LsString* ok; LsString* err; } u; } LsRes_String_String;
    /* tag 0 = Ok, tag 1 = Err. */

Generic `Vec<T>` is a by-value struct emitted by codegen
(`{ size_t len; size_t cap; T* data; }`); its buffer is a raw `malloc` array,
not reference counted. The codegen copy helper deep-copies buffer and elements;
the release helper releases owned elements and frees the buffer.

## Ownership cookbook (for generated code)

A String is always an owned `LsString*` handle unless it is a borrowed
parameter/field. Pass one reference in and take one reference out.

- Fresh value: `ls_str_new()`, `ls_str_cstr("text")`,
  `ls_str_from_parts(bytes, len)`, `ls_format_begin(hint)` etc. all return an
  owned handle (count 1).
- Copy a handle: `copy = (LsString*)ls_retain((void*)original);`.
- Drop a handle: `ls_release(handle);`.
- String mutators are copy-on-write and **consume the caller's reference**;
  they return the handle to keep:
  `s = ls_str_push_str(s, suffix);`, `s = ls_str_push_cstr(s, ",");`,
  `s = ls_str_push_char(s, 'x');`, `s = ls_str_clear(s);`.
  When the receiver is unique and has room the mutation happens in place;
  otherwise it detaches (allocates a new object, releases the old reference).
- Format builder: start with `acc = ls_format_begin(hint);` then chain the
  consumers and keep the final result:
  `acc = ls_fmt_i64(acc, value);`, `acc = ls_format_push_cstr(acc, ", ");`,
  `acc = ls_fmt_f64(acc, x);`, `acc = ls_format_push_char(acc, '\n');`.
- `Vec<T>` push / insert, with the inline fast path:

      /* good: one branch in the common (capacity available) case */
      ls_vec_push_bytes(&v.data, &v.len, &v.cap, &value, sizeof value);

      /* still supported, out-of-line helper with a safe aliasing slow path */
      ls_vec_insert_bytes(&v.data, &v.len, &v.cap, v.len, &value, sizeof value);

  Both grow geometrically (start 4, double) and handle a `value` that points
  into the vector buffer. `ls_vec_alloc(capacity, elem_size)` allocates an
  exact-capacity zeroed buffer (for `Vec::with_capacity`), `ls_vec_free_bytes`
  frees it.
- `Vec<String>` owns one reference per element: `ls_vec_string_push(v, s)`
  retains `s`, `ls_vec_string_get` retains into `*out`, `ls_vec_string_remove`/
  `pop` transfer ownership out, `ls_vec_string_clear` releases elements and
  keeps the buffer, `ls_vec_string_free` releases elements and the buffer.
- Option/Result: `LsOpt_String r = ls_opt_string_some(s);` moves the reference
  into the union; the inactive branch is initialized, so release exactly the
  active branch once. Same for `ls_res_string_ok` / `ls_res_string_err`.
- Printing: `ls_print(s)` / `ls_println(s)` / `ls_eprintln(s)` take a borrowed
  handle (they do not consume it) and flush after every call, so program output
  keeps its order when pipes interleave stdout and stderr.
- Program exit: `ls_exit(code)` flushes stdout/stderr and exits; generated
  `main` calls `ls_runtime_shutdown()` before returning normally.

## Formatting

- `Display` of floats is the shortest round-trip decimal, always expanded
  (never exponent notation): `1.0` prints `1`, `-0.0` prints `-0`,
  `1e300` prints 301 digits.
- `Debug` of floats adds `.0` to integral values and switches to exponent
  notation when the decimal exponent is below -4 or at least 16
  (`1e15` -> `1000000000000000.0`, `1e16` -> `1e16`, `1e-4` -> `0.0001`,
  `1e-5` -> `1e-5`); `NaN` and `inf`/`-inf` are spelled like Rust.
- `f32` values are formatted from their own shortest representation.
- Debug String escaping: `"quote"`, backslash, `\n`, `\r`, `\t` and control
  bytes as `\u{XX}`; UTF-8 bytes pass through unchanged.
- Parsing: `ls_str_parse_i64` / `ls_str_parse_f64` consume the whole string,
  reject whitespace/underscores, return false on overflow; `parse_f64` accepts
  Rust's forms (`1.`, `.5`, `1e10`, `inf`, `NaN`).

## Panics and checked arithmetic

All panics print to stderr

    thread 'main' panicked at <file>:<line>:<col>:
    <message>

then flush stdout and stderr and call `exit(101)`. They never return.

| Helper | Message |
| --- | --- |
| `ls_panic_div_zero(file,line,col)` | `attempt to divide by zero` |
| `ls_div_<T>_checked` (divisor 0) | `attempt to divide by zero` |
| `ls_rem_<T>_checked` (divisor 0) | `attempt to calculate the remainder with a divisor of zero` |
| `ls_panic_overflow(file,line,col,op)` | `attempt to <op> with overflow` (`add`, `subtract`, `multiply`, `divide`, `negate`, `shift left`, `shift right`) |
| `ls_panic_bounds(file,line,col,len,index)` | `index out of bounds: the len is <len> but the index is <index>` |
| `ls_panic_at` / `ls_panic_cstr` | user message |

Checked arithmetic helpers exist for every integer type (`ls_add_i8_checked`,
..., `ls_neg_i64_checked`, `ls_shl_u64_checked`, ...). They take the two
operands, then `file, line, col`, panic exactly like Rust, and are emitted only
in debug builds; release builds use wrapping operators. Divide/remainder by
zero panics in both configurations because generated code always calls the
checked helper for division.

## IO, process and math

- `ls_runtime_init(argc, argv)`; `ls_args()` returns `LsVec_String` of the
  user arguments **excluding** the executable name.
- `ls_read_line()` returns `LsOpt_String`: `None` on EOF, otherwise the line
  with the trailing newline (and Windows `\r`) removed.
- `ls_read_file(path)` / `ls_write_file(path, contents)` return
  `LsRes_String_String`; errors carry the OS message as
  `"<strerror> (os error N)"`. Both are binary safe.
- `ls_exit(code)` flushes and exits with `code`.
- Math helpers: `ls_abs_*`/`ls_min_*`/`ls_max_*` for every integer type,
  plus `ls_abs_f32/f64`, `ls_min_f32/f64`, `ls_max_f32/f64`, `sqrt`, `pow`,
  `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `ln`, `log10`, `exp`.

## Leak reporting

When the runtime is compiled with `LSACA_DEBUG` or `LSACA_DEBUG_RC`,
`ls_runtime_shutdown()` runs pending collections and then walks the live-object
list. If anything is still alive it prints to stderr:

    lsaca: warning: 3 reference-counted allocation(s) leaked
    lsaca:   leaked allocation of 10 bytes
    ...

Up to eight allocations are listed (most recently allocated first). A correct
program prints nothing, also in debug builds; release builds do not track
allocations at all. The runtime self-test uses this mode to prove that the
whole suite leaks nothing.

## Tests and benchmarks

Runtime suite (release; use `-DLSACA_DEBUG -DLSACA_DEBUG_RC` for the debug
configuration; run from this directory):

    gcc -std=c17 -O2 -DNDEBUG -Wall -Wextra -Wpedantic -I. \
        test/main.c lsaca_rt.c lsaca_rt_string.c lsaca_rt_vec.c lsaca_rt_fmt.c \
        -lm -o smoke
    ./smoke

It prints `print works` and ends with `ALL CHECKS PASSED (411 checks)`.

Windows smoke matrix (all configurations, panic/leak/IO/stdin/args child
processes):

    powershell -NoProfile -ExecutionPolicy Bypass -File runtime/test/run_smoke.ps1

Benchmarks (best-of-three wall clock per section):

    gcc -std=c17 -O2 -DNDEBUG -Wall -Wextra -Wpedantic -I. \
        test/bench_runtime.c lsaca_rt.c lsaca_rt_string.c lsaca_rt_vec.c \
        lsaca_rt_fmt.c -lm -o bench
    ./bench

The Rust embedding contract is tested by:

    rustc --edition 2021 --test crates/lsaca/src/runtime.rs -o runtime_files_test
    ./runtime_files_test

## Portability notes

- C17 only; `max_align_t`, `stdint.h`, `stdbool.h`, `<math.h>`, `<stdio.h>`.
- Windows: stdout/stderr are switched to binary mode so program output is
  LF-only (matching Rust); file IO uses the C library and is binary safe.
- No hidden global state besides the stored argument vector and (in tracking
  builds) the intrusive debug list of live allocations.
- No threads, no signals; everything is deterministic.
