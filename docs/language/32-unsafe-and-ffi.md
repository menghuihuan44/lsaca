# Unsafe code and C FFI

lsaca 2.5.0 adds a deliberately small unsafe layer and a direct C FFI. The
safe subset, value semantics and the ORC memory model are unchanged: `unsafe`
is a compile-time permission for the few operations the safe subset cannot
express, not a second language and not a runtime mode. The compiler itself is
still written in safe Rust and keeps `#![forbid(unsafe_code)]`.

This chapter is the safety reference. It states what the compiler checks, what
the programmer promises, which types may cross the C boundary, and the ORC
invariants that unsafe code must preserve. The standard-library pages for
[`ptr`](../stdlib/ptr.md), [`cstring`](../stdlib/cstring.md),
[`cstr`](../stdlib/cstr.md) and [`mem`](../stdlib/mem.md) list the API.

## What unsafe means

Most of lsaca remains checked exactly as before. An unsafe context only permits
the operations in this table; everything else in the same block is ordinary
safe code.

| Operation | Needs unsafe | Diagnostic without it |
| --- | --- | --- |
| Reading through a raw pointer, `*p` | yes | `E0133` |
| Writing through a raw pointer, `*p = value` | yes | `E0133` |
| Calling a function declared in an `extern "C"` block | yes | `E0133` |
| `MaybeUninit::assume_init` | yes | `E0133` |
| `CStr::from_ptr` | yes | `E0133` |
| `CString::from_raw` | yes | `E0133` |
| Constructing, reading or writing a union | yes | `E0133` |

Creating a raw pointer, comparing raw pointers with `==`/`!=`, and calling
`is_null`/`addr` are safe. Calling an `unsafe fn` requires an unsafe context
first: the function signature carries that requirement. A violation is `E0133`
and the note names the operation.

There are two syntactic forms. An `unsafe { ... }` **block** is an ordinary
block expression with permission to perform unsafe operations; its value is the
block value and it adds no instructions:

```ls
fn main() {
    let mut value: i64 = 40;
    let raw = &mut value as *mut i64;
    unsafe {
        *raw = *raw + 2;
    }
    println!("{}", value);
}
```

Output:
```text
42
```

An `unsafe fn` may perform unsafe operations in its body without an inner
block. Free functions and inherent impl methods may be declared `unsafe fn`,
optionally with visibility (`pub unsafe fn`); trait methods cannot be `unsafe fn`
in 2.5.0, because trait dispatch cannot require an unsafe caller context. The
function signature makes every call an unsafe operation. Its callers must be in
an unsafe context:

```ls
unsafe fn double_at(raw: *const i64) -> i64 {
    *raw * 2
}

fn main() {
    let value: i64 = 21;
    let raw = &value as *const i64;
    println!("{}", unsafe { double_at(raw) });
}
```

Output:
```text
42
```

The C backend lowers an unsafe block and an unsafe function to the same
statements it emits for safe code. Unsafe changes what the type checker accepts,
never what the runtime does. In particular it does **not** disable ORC, does not
change moves or `Drop`, and does not make a managed value ignorable.

## The safety contract

The compiler checks the *form* of unsafe code; the programmer promises that the
values and interfaces are valid. Both halves are required.

The compiler checks:

* that each operation from the table above appears inside an unsafe context
  (`E0133`);
* that an `extern "C"` signature contains only FFI-safe types (`E0770`);
* that `#[repr(C)]` is on a struct or union, with the argument `C` (`E0771`);
* that union fields are FFI-safe scalars, pointers, unions or `repr(C)` structs
  and carry no `Drop` (`E0772`);
* that a union is not used where the model forbids it: no `Drop` impl, no
  methods, no destructuring patterns, no generated equality or formatting
  (`E0773`);
* all the ordinary type, ownership, move and borrow rules around the unsafe
  code. Unsafe does not silence the borrow checker.

The programmer promises:

* **Validity.** A raw pointer that is dereferenced points to a live, properly
  aligned, initialized value of the pointee type. The compiler checks neither
  validity nor alignment nor initialization.
* **Extent and lifetime.** The allocation outlives every use of the pointer.
  Unsafe code can create a dangling pointer, and using one is outside the
  contract: it may crash, read stale bytes or corrupt memory.
* **Mutability and aliasing.** No `&mut T` to the same place is live while the
  pointer is used for writing, and no shared reference is used while a write
  through the pointer is in progress. The raw pointer itself carries no
  lifetime, so this obligation is entirely on the programmer.
* **One owner.** `CString::from_raw` receives only a pointer produced by
  `CString::into_raw` that has not already been reclaimed; ownership moves back
  exactly once. The lsaca allocator is not the C allocator, so a pointer
  obtained from C `malloc` must not be passed to `from_raw`.
* **C agreement.** An `extern "C"` declaration matches the real C symbol:
  name, parameter count and types, return type, calling convention, and whether
  it may return null. The compiler emits a prototype, so a mismatch is not
  diagnosed at the call site.
* **Active union field.** The compiler does not track which union field is
  active. Constructing and reading are both unsafe; interpreting the overlapped
  bytes as a different field is the programmer's C-level decision.
* **Initialized `MaybeUninit`.** `assume_init` is called only after a value was
  written into the storage (or after C filled it). Dropping an initialized
  `MaybeUninit` whose type has `Drop` leaks instead of running the destructor;
  it never double-frees.

### ORC invariants

Reference counting is automatic and remains automatic in unsafe code. Unsafe
code must preserve these invariants:

1. **Never release or retain an ORC object by hand.** There is no public
   primitive that takes over a reference count, and unsafe code must not call
   runtime helpers directly or emit the operation in C.
2. **Never forge or rely on a reference count.** Do not copy the internal
   header of a `String`, `Vec`, `Rc`, `Box`, `HashMap` or closure; do not
   assume a particular count, and never reconstruct a managed value from raw
   bytes.
3. **Raw pointers never own.** A `*const T` or `*mut T` field, local or
   temporary does not keep its pointee alive, does not participate in drop
   order, and is not traced by the cycle collector. It is a trivial copy.
4. **Managed values never cross the FFI boundary.** The FFI-safe type list
   excludes every managed type; a call cannot transfer or share an ORC object,
   so no C function can decrement a count, retain a value or free an lsaca
   buffer. Pass raw bytes or C-compatible aggregates instead, and keep the
   owner on the lsaca side for the duration of the call.

The C backend emits `const T*`/`T*` for pointer-typed values and no retain,
release, copy hook or trace hook, so these invariants are also visible in the
generated code.

## Raw pointers

The pointer types are `*const T` and `*mut T`. `T` must be sized: `*const
i64` and `*mut [u8; 4]` are fine, `*const str` and `*mut [i64]` are rejected
because the pointee has no known size.

| Property | Rule |
| --- | --- |
| Copy | Yes. A pointer is a plain value; `let copied = p;` does not move the pointee and leaves `p` usable. |
| ORC | No. Pointers are never counted, never traced and have no drop glue. |
| Lifetime | None. The compiler does not associate the pointer with the borrow it came from. |
| Aggregates | Legal in structs, enums, tuples, arrays and `Vec` elements, which is what C data layouts need. |
| Dereference | `*p`, and `p.field` after a dereference, need unsafe and are not auto-dereferenced like references. |
| Arithmetic | `add`/`sub` methods only; the `+` and `-` operators are not defined for pointers. |

### Creating a pointer

| Form | Result |
| --- | --- |
| `&value as *const T` | Pointer to the place. |
| `&mut value as *mut T` | Mutable pointer to the place. |
| `p as *const U`, `p as *mut U` | Pointer-to-pointer cast. |
| `p as usize`, `n as *const T`, `n as *mut T` | Pointer-to-integer and integer-to-pointer cast. |
| `std::ptr::null::<T>()` | Null `*const T`. |
| `std::ptr::null_mut::<T>()` | Null `*mut T`. |

`std::ptr::null` is implemented with the integer-to-pointer cast, as
`0 as *const T`. A pointer cast changes the type of the pointer, never the
pointee and never the address.

### Operations

| Operation | Result | Unsafe |
| --- | --- | --- |
| `p.is_null()` | `bool`, true for the all-zero address. | no |
| `p.addr()` | `usize` address value. | no |
| `p == q`, `p != q` | Compare addresses, not pointees. | no |
| `p.add(count)`, `p.sub(count)` | New pointer `count` elements forward or back. `count` is `usize`. | yes |
| `*p` | Read the pointee. | yes |
| `*p = value` | Write the pointee. | yes |

A raw pointer does not auto-dereference: `p.field` is not a field access on the
pointee, and `p.add(1)` is a method call on the pointer, not arithmetic on the
pointee. Use `(*p).field` or `unsafe { *p }` explicitly.

`add` and `sub` are unsafe because they can leave the allocation even when the
result is never dereferenced. The pointer must stay in bounds of the same
allocation for the operation to be valid; using the result to forge a pointer
outside the allocation and later dereferencing it is undefined.

```ls
fn main() {
    let values: Vec<i64> = vec![10, 20, 30];
    let base = values.as_ptr();
    let second = unsafe { base.add(1) };
    unsafe {
        println!("{}", *second);
    }
    println!("{}", base.is_null());
}
```

Output:
```text
20
false
```

A null pointer is a valid pointer value. `is_null` is safe, but dereferencing or
arithmetic on null is not allowed by the contract:

```ls
fn main() {
    let empty = std::ptr::null::<i64>();
    let empty_mut = std::ptr::null_mut::<i64>();
    println!("{} {}", empty.is_null(), empty_mut.is_null());
}
```

Output:
```text
true true
```

### Pointers into managed buffers

`String::as_ptr`, `Vec::as_ptr` and `Vec::as_mut_ptr` expose the underlying
buffer as `*const u8` or `*mut T`. The pointer is valid only while the owner is
alive and the borrow that produced it is in effect; mutating the owner may
reallocate the buffer and invalidate the pointer immediately. A raw pointer does
not extend the borrow, so it is the caller's job to freeze the owner while C
uses the buffer.

## `extern "C"` declarations

An `extern` block declares C functions. Only the `"C"` ABI exists in 2.5.0:

```ls
extern "C" {
    fn strlen(text: *const u8) -> usize;
}

fn main() {
    let text = String::from("lsaca");
    let length = unsafe { strlen(text.as_ptr()) };
    println!("{}", length);
}
```

Output:
```text
5
```

Declarations in the block have no body, no generics, no receiver and are not
written with `unsafe`; they may be `pub`. The unsafe permission belongs to the
call site, not to the declaration. The block is grouping syntax: every declared
function is an ordinary item for name resolution. An unknown ABI string, a declaration with a body, a
malformed block or a duplicate name is `E0001`. The standard library's
`extern fn __intrinsic(...)` form is separate and remains restricted to `std`;
user code that writes an intrinsic-style `extern fn` gets `E0001`, and a user
function whose name collides with an intrinsic is also `E0001`.

Calls require an unsafe context. The backend emits a prototype at the top of the
generated C file, for example `extern size_t strlen(const uint8_t*);`, and
calls the symbol directly with no shim and no name mangling. The generated
project already links the C library and `libm`; there is no `#[link]` attribute
in 2.5.0, so a function from another library cannot be bound yet.

### FFI-safe types

A parameter or return type in an `extern "C"` declaration must be FFI-safe.
Anything else is `E0770`:

| Type | Allowed | Notes |
| --- | --- | --- |
| Integer types, `isize`, `usize` | yes | The corresponding C integer. |
| `f32`, `f64` | yes | C `float`/`double`. |
| `bool` | yes | C `bool`/one byte; agree with the C declaration. |
| `char` | yes | A Unicode scalar value, 32 bits in the current ABI. |
| `*const T`, `*mut T` | yes | Pointers to FFI-safe `T`; the pointer itself is always safe ABI. |
| `&T`, `&mut T` over FFI-safe `T` | yes | Borrowed for the call; the pointee must stay valid and follow the aliasing rule. |
| `()` | yes | No value. |
| `#[repr(C)]` structs and unions | yes | By value or through a pointer when every field is FFI-safe. |
| `String`, `Vec`, `Rc`, `Box`, `HashMap` | no | Managed ORC types never cross the boundary. |
| `Option`, `Result`, tuples, enums, `dyn` | no | Not C-compatible in 2.5.0, even when every payload is scalar. |
| `&[T]`, `&str`, `CString`, `CStr`, `MaybeUninit` | no | Pass a raw pointer and a length instead. |
| Structs without `#[repr(C)]` | no | Their layout is unspecified and not part of the ABI. |

A `repr(C)` struct is FFI-safe only when every field is FFI-safe; the
declaration is rejected otherwise. Pointers inside an FFI-safe struct may point
to managed data, but the struct does not own it and the call must not let C
retain the pointer.

## `#[repr(C)]` structs

The attribute is written on the declaration:

```ls
#[repr(C)]
struct DivT {
    quot: i32,
    rem: i32,
}

fn main() {
    let value = DivT { quot: 7, rem: 1 };
    println!("{} {}", value.quot, value.rem);
}
```

Output:
```text
7 1
```

Only a `struct` or `union` may carry `#[repr(C)]`, and only with the argument
`C`. Any other attribute placement, an unknown argument, or `#[repr(C)]` on an
enum, function or field is `E0771`. A `repr(C)` struct has C layout: fields in
declaration order with the C compiler's alignment and padding. Its size and
alignment are part of the language contract and match the platform C compiler.

A struct without `#[repr(C)]` has an unspecified layout. The current backend
happens to emit fields in declaration order, but no program may depend on that,
and a non-`repr(C)` struct is never FFI-safe.

```ls
#[repr(C)]
struct DivT {
    quot: i32,
    rem: i32,
}

extern "C" {
    fn div(numerator: i32, denominator: i32) -> DivT;
}

fn main() {
    let result = unsafe { div(17, 5) };
    println!("{} {}", result.quot, result.rem);
}
```

Output:
```text
3 2
```

Struct fields are private to their module as usual; an extern declaration in
another module can still use the type by name if the type is public. Methods,
`Drop` and normal value semantics are unchanged for `repr(C)` structs; the
attribute controls layout only.

## Unions

A union overlays its fields at offset zero:

```ls
union Number {
    integer: i64,
    float: f64,
}

fn main() {
    let parts = unsafe { Number { integer: 1 } };
    let integer = unsafe { parts.integer };
    println!("{}", integer);
}
```

Output:
```text
1
```

Every field starts at offset 0. The size of the union is the maximum field size
and its alignment is the maximum field alignment, exactly as in C. A union has
C layout by definition, so `#[repr(C)]` on a union is accepted as a no-op for
readability.

`union` is a reserved keyword in 2.5.0, so it can no longer be an identifier.
That is why the `HashSet` set-algebra method previously called `union` is now
`union_with`.

Field types are restricted to:

* FFI-safe scalar types, including integers, floats, `bool` and `char`;
* raw pointers;
* other unions;
* `#[repr(C)]` structs.

A managed type (`String`, `Vec`, `Rc`, `Box`, `HashMap`, and so on), any enum
(including one with only scalar payloads), or any other forbidden field type is
`E0772`. Generic unions are not supported.

A union has no methods, no `Drop` impl, no destructuring patterns and no
generated equality or formatting. Using it in any of those ways is `E0773`.
Construction, reading and writing are unsafe, because the compiler does not
track which field is active. A union is emitted as a plain C union, so it can
be passed by value or through a pointer across the C boundary.

Because every field is FFI-safe and unmanaged, a union cannot hide an ORC
reference count. If a union is used to reinterpret bytes, the programmer is
responsible for the interpretation, just as in C.

## `CString`, `CStr` and `MaybeUninit`

### `CString`

`CString` owns a NUL-terminated byte buffer:

| Operation | Signature | Notes |
| --- | --- | --- |
| `new` | `CString::new(text: &str) -> Result<CString, NulError>` | Copies the text and appends NUL; an embedded NUL is `Err`. |
| `as_ptr` | `as_ptr(&self) -> *const u8` | Borrowed pointer, valid while the `CString` lives and is not moved or changed. |
| `len` | `len(&self) -> usize` | Byte length excluding the trailing NUL. |
| `as_bytes` | `as_bytes(&self) -> &[u8]` | Read-only bytes without the trailing NUL. |
| `into_raw` | `into_raw(self) -> *mut u8` | Gives up ownership; the buffer is not freed. |
| `from_raw` | `unsafe CString::from_raw(ptr: *mut u8) -> CString` | Takes ownership back; only a pointer from `into_raw` that has not been reclaimed. |

`CString` is an ordinary managed value: it is move-only, reference counted,
and has drop glue that releases its buffer. A raw pointer obtained from
`as_ptr` does not keep it alive. `from_raw` is unsafe because the compiler
cannot know whether the allocation is still valid or whether it came from
`into_raw`.

```ls
use std::cstring::CString;
use std::cstr::CStr;

extern "C" {
    fn strlen(text: *const u8) -> usize;
}

fn main() {
    let owned = CString::new("hello").unwrap();
    let length = unsafe { strlen(owned.as_ptr()) };
    let borrowed = unsafe { CStr::from_ptr(owned.as_ptr()) };
    println!("{} {} {}", length, borrowed.len(), borrowed.to_str().unwrap());
}
```

Output:
```text
5 5 hello
```

The lsaca allocator is not the C allocator. A buffer returned by C `malloc`
must never be passed to `CString::from_raw`, and an lsaca `CString` buffer must
not be passed to C `free`. A C function may read the bytes during the call; it
must not retain them after the call returns.

### `CStr`

`CStr` is a borrowed, NUL-terminated view. It owns nothing and never frees:

| Operation | Signature | Notes |
| --- | --- | --- |
| `from_ptr` | `unsafe CStr::from_ptr(ptr: *const u8) -> CStr` | Wraps the buffer in a `CStr` value; the buffer must be NUL-terminated and valid. |
| `len` | `len(&self) -> usize` | Byte length excluding the terminating NUL. |
| `to_str` | `to_str(&self) -> Result<&str, Utf8Error>` | Validates UTF-8 and returns the borrowed text. |

`CStr::from_ptr` is unsafe because the compiler cannot check that the pointer is
valid, NUL-terminated or still alive. The returned `CStr` is a small value that
does not extend the life of the buffer; `to_str` borrows that value and the
buffer. `to_str` is the checked conversion to text; reading the bytes without
checking UTF-8 is the caller's decision.

### `MaybeUninit<T>`

`MaybeUninit<T>` is a builtin layout wrapper whose size and alignment are
exactly those of `T`. It is used for out parameters and for storage that C
fills:

| Operation | Signature | Unsafe |
| --- | --- | --- |
| `uninit` | `MaybeUninit::<T>::uninit() -> MaybeUninit<T>` | no |
| `write` | `write(&mut self, value: T)` | no |
| `as_mut_ptr` | `as_mut_ptr(&mut self) -> *mut T` | no |
| `assume_init` | `assume_init(self) -> T` | yes |

`assume_init` takes the value out of the wrapper; the caller promises that a
value was written first. Dropping a `MaybeUninit<T>` after a write does not run
`T`'s destructor. For a type with `Drop`, that is a deliberate leak, not a
double free; call `assume_init` and let the value drop if cleanup is needed.

A typical out parameter:

```ls
use std::mem::MaybeUninit;

extern "C" {
    fn frexp(value: f64, exponent: *mut i32) -> f64;
}

fn main() {
    let mut exponent = MaybeUninit::<i32>::uninit();
    let mantissa = unsafe { frexp(8.0, exponent.as_mut_ptr()) };
    let exponent = unsafe { exponent.assume_init() };
    println!("{} {}", mantissa, exponent);
}
```

Output:
```text
0.5 4
```

The C function writes through the pointer returned by `as_mut_ptr`. No
intermediate value exists, so the storage is not initialized before the call;
`assume_init` is correct only because `frexp` always writes its second
argument.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `E0133` | An unsafe operation appears outside an `unsafe` block or `unsafe fn`; the note names the operation. |
| `E0770` | A parameter or return type in an `extern "C"` signature is not FFI-safe. |
| `E0771` | `#[repr(C)]` is misplaced, repeated, or has an unsupported argument. |
| `E0772` | A union field has a managed or otherwise forbidden type. |
| `E0773` | A union is used in a forbidden way: `Drop`, a method, a destructuring pattern, generated equality or formatting. |
| `E0001` | A malformed `extern` block, an unknown ABI string, or a collision with an intrinsic name. |

The ordinary rules still apply: a type mismatch is `E0308`, use after move is
`E0382`, destructuring a `Drop` value is `E0509`, and borrowing errors keep
their usual codes. See [Error codes](31-error-codes.md) for the full index.

## Limits in 2.5.0

The unsafe layer is intentionally smaller than Rust's. In 2.5.0:

* there are no function pointers or callbacks, so C APIs that take a callback
  cannot be bound;
* there are no variadic declarations, so `printf`-style functions cannot be
  declared;
* there is no `#[link]` and no extra-library support; only the C library and
  `libm` are linked;
* `repr(packed)`, `repr(align)`, `repr(transparent)`, `repr(C)` enums and
  explicit discriminants are not supported;
* there is no inline assembly, no allocator API, and no `Box::into_raw` or
  `Rc` raw interaction;
* managed types never cross the FFI boundary, and references in arbitrary
  aggregates remain unavailable until v2.6;
* `MaybeUninit` does not track initialization, and reading an uninitialized
  value is outside the contract;
* `CString::from_raw` accepts only lsaca-allocated pointers, because there is
  no C allocator bridge without function pointers.

## Writing unsafe code safely

A short checklist before every unsafe block:

1. Name the operation that needs unsafe and keep it as small as possible.
2. State why the pointer is live, aligned and initialized, and who keeps the
   allocation alive.
3. Check mutable aliasing against every reference and pointer in scope.
4. Check the C signature against the header, including nullability and the
   ownership of every pointer parameter.
5. Keep managed values on the lsaca side; pass bytes, lengths and C-layout
   aggregates only.
6. Re-check the ORC invariants: no manual retain/release, no forged counts, no
   owning pointer, no managed value across the boundary.

The acceptance tests for this chapter bind real libc and libm functions, cover
a `repr(C)` struct returned by value, out parameters through raw pointers and
`MaybeUninit`, `CString`/`CStr` round trips, null handling and a union round
trip, and run under the address and undefined-behaviour sanitizers. The negative
tests confirm that managed types are rejected in extern signatures and union
fields, that a raw dereference outside unsafe is `E0133`, and that codegen emits
no ORC operations for pointer-typed values.

## What next

* The standard-library pages: [ptr](../stdlib/ptr.md),
  [cstring](../stdlib/cstring.md), [cstr](../stdlib/cstr.md) and
  [mem](../stdlib/mem.md).
* The unsafe and FFI design: [RFC 0005: controlled unsafe and
  C FFI](../../rfcs/0005-controlled-unsafe-and-ffi.md).
* Memory-model background: [Memory model](05-memory-model.md).
* Deliberate differences from Rust: [Deviations from Rust](deviations.md).
* Known gaps: [Known limitations](limitations.md).
