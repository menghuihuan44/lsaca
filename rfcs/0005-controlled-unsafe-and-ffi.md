# 0005: Controlled unsafe, raw pointers and C FFI

* Status: accepted
* Target release: v2.5
* Author: lsaca maintainers, 2026-11-28

## Summary

lsaca gains a deliberately small, fully auditable unsafe layer: `unsafe` blocks
and functions, raw pointers, `extern "C"` declarations, `#[repr(C)]` structs,
unions, `CString`/`CStr` and `MaybeUninit<T>`. The safe subset, the ORC memory
model and the compiler's own `#![forbid(unsafe_code)]` are unchanged; unsafe is a
compile-time permission for code that the safe subset cannot express, and every
unsafe operation is visible in the source and denied by default.

## Motivation

Every program that must talk to the operating system or to an existing C library
is blocked today. The only extern mechanism is `extern fn __name(...)` in the
standard library, which maps to compiler intrinsics; users cannot declare or call
a C function. Representative blocked programs:

* binding a libc/libm function for a numeric primitive (`frexp`, `tgamma`, `erf`);
* reading a C string returned by the platform (`getenv`, `strerror`);
* passing a C struct by value or receiving one (`div` returning `div_t`);
* laying out a struct the C ABI defines and passing a pointer to it;
* representing a C union (tagged value, socket address storage, variant records).

The workaround today is impossible without a compiler intrinsic, which means
every new C function needs a compiler change. That does not scale and it hides
FFI behind the standard library instead of making it an explicit, auditable user
capability.

## Design

### Unsafe contexts

Two syntactic forms:

```lsaca
let value = unsafe { *raw };
unsafe fn read(raw: *const i64) -> i64 { *raw }
```

An `unsafe { ... }` expression is a block expression: its value is the block
value and it adds no runtime cost. An `unsafe fn` may perform unsafe operations
directly; calling it needs an unsafe context. The operations that require an
unsafe context are:

1. dereferencing a raw pointer (`*p`) and assigning through one (`*p = v`);
2. calling an `extern "C"` function;
3. `MaybeUninit::assume_init`;
4. `CStr::from_ptr` and `CString::from_raw`;
5. constructing, reading or writing a union.

Everything else is safe. A violation is E0133 with a note naming the operation.
The lowering turns unsafe syntax into the same HIR as safe syntax, so the C
backend, the cycle collector and the runtime do not change; unsafe is checked
once, statically.

Safety contract: unsafe does not disable ORC, does not change value semantics and
does not allow managing a reference-counted object by hand. The invariants are
documented in `docs/language/32-unsafe-and-ffi.md`: never call a release
primitive on an ORC value, never forge a reference count, never store a managed
value behind a raw pointer that outlives it, and never let a managed value cross
the FFI boundary. Unsafe code can create dangling pointers; that is the one
promise the compiler stops checking, and it is stated at every use site.

### Raw pointers

Types `*const T` and `*mut T` exist for sized `T`. They are Copy, are never
reference counted or traced by the cycle collector, and may appear in aggregates
(struct fields, tuples, arrays, Vec elements) because that is required for
C-compatible data. They carry no lifetime. Construction:

* `&value as *const T` and `&mut value as *mut T`;
* pointer casts `p as *const U`, `p as *mut U`, `p as usize`, `n as *const T`,
  `n as *mut T`;
* `std::ptr::null::<T>()` and `std::ptr::null_mut::<T>()`, written in the
  standard library as `0 as *const T`.

Operations: `is_null()` and `addr()` are safe; `add(count: usize)` and
`sub(count: usize)` require unsafe because they can leave the allocation;
`*p` reads and `*p = v` writes require unsafe; `==` and `!=` compare
addresses. There is no pointer arithmetic through operators and no dereference
of an integer.

The C representation is the obvious one: `const T*` and `T*` with the mapped
`T`. No retain, release, copy hook or trace hook is generated for pointer
values; they are trivial copies. `String::as_ptr`, `Vec::as_ptr` and
`Vec::as_mut_ptr` expose the underlying buffers for the duration of a call;
the documentation states that the pointer is invalidated by mutation or by the
end of the borrow.

### extern "C" declarations

```lsaca
extern "C" {
    fn strlen(text: *const u8) -> usize;
    fn div(numerator: i32, denominator: i32) -> DivT;
}
```

Only the `"C"` ABI is accepted; other ABI strings are E0001. Declarations have
no body, no generics and no receiver, and may be `pub`. The existing std-only
`extern fn __name(...) -> T;` intrinsic form stays unchanged for compiler
intrinsics and cannot be written outside the standard library; a user function
whose name collides with an intrinsic is E0001.

FFI-safe parameter and return types: integers, floats, `bool`, `char`, raw
pointers, `&T` and `&mut T` when `T` is FFI-safe, `repr(C)` structs and
unions by value or through a pointer, and `()`. Everything else is rejected in
the declaration with E0770: `String`, `Vec`, `Rc`, `Box`, `Option`, `Result`,
tuples, enums, `dyn`, slices, `str`, `HashMap` and structs without
`#[repr(C)]`. The FFI boundary is therefore an ORC boundary by construction:
no refcounted value can be named in an FFI signature, so no ownership rule can
be violated by the call.

Calls require an unsafe context. Codegen must call the platform symbol directly
without a shim or a wrapper function, but it cannot use the symbol name as the C
identifier: the generated C includes the standard headers and builtins that
already declare `strlen`, `memcmp`, `getenv` and friends, and re-declaring them
with lsaca's spellings is a C type conflict. Instead codegen emits one
declaration per extern function whose C identifier is compiler generated
(`lsaca_ffi_<id>`) and whose asm label is the platform symbol:

```c
#if defined(__APPLE__)
#define LSACA_FFI_ASM(name) __asm__("_" #name)
#else
#define LSACA_FFI_ASM(name) __asm__(#name)
#endif
extern size_t lsaca_ffi_12(const uint8_t* text) LSACA_FFI_ASM(strlen);
```

Calls then use `lsaca_ffi_12`. The call is direct at the ABI level (no extra
frame, no wrapper translation unit), it stays compatible with the standard
headers, and the `repr(C)` struct a C library returns by value (for example
`div` returning `div_t`) is layout compatible with the lsaca `repr(C)` struct
declared for it. Darwin needs the leading underscore in the asm label, hence the
macro. The mechanism requires GCC or Clang, the toolchains this backend
supports; other compilers are rejected at C compile time with a clear `#error`.
The generated project already links the C library and libm, so the platform
libraries need no extra configuration. There is no `#[link]` attribute in v2.5:
linking additional libraries is a packaging feature for a later release.

### repr(C)

```lsaca
#[repr(C)]
struct DivT {
    quot: i32,
    rem: i32,
}
```

The attribute is accepted on structs (and as a no-op on unions) only with the `C`
argument; any other placement or argument is E0771. A `repr(C)` struct has C layout: fields in
declaration order with the C compiler's alignment and padding, and its size and
alignment are part of the language contract. Structs without `repr(C)` keep an
unspecified layout (the current backend emits declaration order, but no program
may depend on it). A `repr(C)` struct is FFI-safe when every field is FFI-safe.
Unions always have C layout, so `#[repr(C)]` on a union is accepted as a no-op
for readability.

### Unions

```lsaca
union Number {
    integer: i64,
    float: f64,
}

let parts = unsafe { Number { integer: 1 } };
let bits = unsafe { parts.integer };
```

All fields overlay at offset 0; size and alignment are the maximum over fields.
Field types are restricted to FFI-safe scalar, pointer, union and `repr(C)`
struct types: managed types are E0772, and `Drop` types, destructuring patterns,
methods and generated equality/formatting are rejected with E0773. Construction,
reads and writes happen inside unsafe. A union is a plain C union in the
generated code, so it can be passed by value or through a pointer across FFI.

### CString, CStr, MaybeUninit

* `CString::new(text: &str) -> Result<CString, NulError>` allocates a copy with a
  trailing NUL; an embedded NUL is an error. `as_ptr` returns `*const u8`;
  `into_raw` and unsafe `from_raw` move ownership through a raw pointer. Only
  pointers obtained from `into_raw` may be given to `from_raw` (the lsaca
  allocator is not the C allocator).
* `CStr::from_ptr(ptr: *const u8) -> &CStr` is unsafe and requires a valid
  NUL-terminated buffer; `to_str() -> Result<&str, Utf8Error>` and `len()` read it.
  It never frees.
* `MaybeUninit<T>` is a builtin type whose size and alignment equal `T`'s.
  `uninit()`, `write(value)` and `as_mut_ptr()` are safe;
  `assume_init() -> T` is unsafe. Dropping an initialized `MaybeUninit` leaks
  instead of running `Drop`; that is documented and never a double free.

`CString`/`CStr`/`MaybeUninit` live in the standard library modules
`std::cstring`, `std::cstr` and `std::mem`; the implementations use only the
language surface described above, except `MaybeUninit`'s layout wrapper, which is
a builtin type so size and alignment are guaranteed.

### Diagnostics

| Code | Meaning |
| --- | --- |
| E0133 | unsafe operation outside an unsafe context |
| E0770 | type is not FFI-safe in an extern signature |
| E0771 | unsupported or misplaced repr attribute |
| E0772 | union field has a managed or otherwise forbidden type |
| E0773 | union used in a way the model forbids (Drop, method, pattern) |

Existing codes cover the rest: E0001 for malformed extern blocks and unknown
ABI strings, E0308 for type mismatches, E0382/E0505/E0509 for ownership and Drop
errors that reach FFI-adjacent code.

## Drawbacks

* A second semantic layer: unsafe code can break the guarantees the safe subset
  gives (dangling pointers, double frees through C, reading uninitialized
  memory). The compiler cannot check these; tests and sanitizers are the
  evidence.
* Raw pointers in aggregates partially reopen what the aggregate-reference
  restriction closed. They carry no lifetime, so 'I stored a pointer' is not
  checked at all.
* C layout freezes what the backend may do with `repr(C)` structs and stops
  layout optimizations there.
* More surface in typeck, codegen and the standard library, and five new error
  codes to document and test.
* The acceptance suite is platform C libraries by necessity: a vendored C
  library would make CI depend on an external build system, which the project
  does not allow.

## Alternatives

* **No unsafe, intrinsics only.** Rejected: every C function would need a
  compiler change, and the standard library would forever be the only FFI
  owner. It also contradicts the v2.x direction of a tool-grade language.
* **Full Rust-level unsafe (function pointers, transmute, allocators, inline
  assembly).** Rejected for v2.5: the surface, the documentation and the
  verification cost exceed one release, and each piece has a clear later home
  (function pointers with callback support, allocators with a memory-model
  release).
* **Safe wrappers only (bind C in the standard library, no user extern).**
  Rejected: it does not remove the compiler change per function and keeps users
  from binding their own platform APIs.
* **A separate `unsafe` sublanguage with a different syntax.** Rejected: unsafe
  is an annotation on ordinary code, not a second language; the C backend sees
  the same statements.

## Compatibility and migration

All additions. No existing program changes meaning: `unsafe`, `union`,
`extern "C"` blocks, attributes and pointer types are new syntax, and the
existing std-only `extern fn __intrinsic` form is unchanged. New keywords
`unsafe` and `union` therefore cannot be identifiers any more, which is a
source-breaking change for programs that used those words as names; it is
documented in the CHANGELOG and the compiler emits a clear parse error. No
deprecation window applies to keyword introduction because the language is
pre-1.0 in its user base; the version policy still records the change.

## Unresolved questions

* Function pointers and callbacks: needed for every event-loop and qsort-style
  API. Candidate for v2.7 or later, together with `repr` interaction.
* `MaybeUninit` and drop obligations: a future release may track
  initialized-ness; v2.5 documents the leak.
* `CString` allocator mismatch: `from_raw` accepts only lsaca-allocated
  pointers today; a C-allocator bridge (taking a free function pointer) is out
  of scope until function pointers exist.
* `#[link]` and build-script-free linking: needed for real third-party
  libraries, deferred.
* Whether `unsafe` should also gate pub access to unsafe invariants (`unsafe
  traits`); no user trait can be unsafe in v2.5.