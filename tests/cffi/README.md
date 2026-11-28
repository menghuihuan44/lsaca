# C FFI acceptance suite

This directory holds the acceptance evidence for the v2.5 safe/unsafe and FFI
contract. The programs are real programs: they bind platform libc and libm
symbols, not test-only shims.

## Layout

The programs live beside the other acceptance fixtures, under
`crates/lsaca/tests/programs/cffi/`:

- `*.ls` — positive programs. Each has a sibling `.stdout` file with the
  exact expected standard output, byte for byte (a trailing newline is
  expected after the last line).
- `negative/*.ls` — programs that must be rejected by the front end.
- `tests/cffi/expectations.txt` — the diagnostic code each negative program must
  produce, plus an optional stable message fragment.
- `run_sanitizers.sh` — emits each positive program with `lsaca emit-c`,
  compiles it with GCC using `-fsanitize=address,undefined`, runs it, and
  compares stdout. It then requires every negative program to fail with its
  recorded code. It is the script the `sanitizers` CI job runs.

A positive check fails when `emit-c` fails, the sanitizer build fails, the
program exits non-zero, stderr is non-empty (sanitizer reports land there), or
stdout differs from the golden file. A negative check fails when the compiler
accepts the program, or when the recorded diagnostic code or message fragment
is missing.

## Positive coverage

| Program | Real symbols | What it proves |
| --- | --- | --- |
| `ffi_libc_strings` | `strlen`, `strcmp`, `memcmp`, `getenv` | `CString` pointers cross into C; `CStr` reads them back; a missing `getenv` result is a null raw pointer. |
| `ffi_div` | `div` | A `repr(C)` struct is returned by value from C (quot/rem layout). |
| `ffi_libm` | `frexp`, `tgamma`, `erf` | Raw-pointer and `MaybeUninit` out-parameters and double returns. |
| `ffi_union` | — | `i64`/ `f64` union overlay; construction and reads in `unsafe`. |
| `repr_union_ok` | — | `#[repr(C)]` on a union is accepted as a no-op and roundtrips. |
| `ffi_cstring` | — | `CString` -> `CStr` -> `into_raw`/`from_raw` ownership roundtrip. |

## Negative coverage

| Program | Code | Contract |
| --- | --- | --- |
| `unsafe_deref` | E0133 | Raw-pointer dereference outside `unsafe`. |
| `unsafe_extern_call` | E0133 | Extern C call outside `unsafe`. |
| `ffi_managed_string_arg` | E0770 | `String` parameter in an extern signature. |
| `ffi_managed_return` | E0770 | `Vec<u8>` return in an extern signature. |
| `repr_enum` | E0771 | `repr(C)` on an enum is rejected. |
| `repr_bad_arg` | E0771 | `repr(align(8))` is unsupported. |
| `repr_on_fn` | E0771 | `repr(C)` on a function is misplaced. |
| `union_managed_field` | E0772 | A managed `String` field in a union. |
| `union_method` | E0773 | A method on a union. |
| `union_pattern` | E0773 | Destructuring a union with a pattern. |
| `extern_non_c_abi` | E0001 | An ABI other than `"C"`. |

## Toolchain note

Binding C symbols uses a generated C identifier plus a GCC/Clang `asm` label
that names the real platform symbol. This keeps generated code warning-free in
translation units that already include `<string.h>`/`<stdlib.h>`. The backend
therefore requires GCC or Clang; MSVC toolchains are not supported.

## Running

From the repository root:

```bash
cargo build --release -p lsaca-cli
bash tests/cffi/run_sanitizers.sh
```

On Windows PowerShell the same script runs under WSL (or Git Bash) with a
POSIX GCC. The e2e suite and the generated-C checker wire the positive
programs separately; this directory is the sanitizer evidence and the source
of truth for expected stdout.

## Integration status

The positive programs are discovered by
`cffi_acceptance_programs` in `crates/lsaca/tests/e2e.rs` and compared
byte-for-byte with their sibling `.stdout` files. The negative programs are
covered by `tests/cffi/run_sanitizers.sh` and `expectations.txt`. The
sanitizer CI job runs the script directly, and the files here are deliberately
self-contained so the suite can also be run on its own.
