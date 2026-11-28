# cstr: CStr

**Status.** Added in 2.5.0. `CStr` is a borrowed, NUL-terminated byte view. It
owns nothing, never frees and never extends the lifetime of the buffer it
points at. It is defined in `stdlib/src/cstr.ls` as `std::cstr::CStr`; the
conversion error is `Utf8Error`.

`CStr` is the read side of C text. The pointer comes from an lsaca `CString`,
a static C string, or a buffer that C filled; `from_ptr` only creates the view,
and everyone involved must keep the bytes alive and unchanged for as long as the
borrow is used.

## API

| Operation | Signature | Meaning |
| --- | --- | --- |
| `from_ptr` | `unsafe CStr::from_ptr(ptr: *const u8) -> CStr` | wraps a NUL-terminated buffer in a `CStr` value; the pointer must be valid and terminated |
| `len` | `len(&self) -> usize` | byte length excluding the terminating NUL |
| `to_str` | `to_str(&self) -> Result<&str, Utf8Error>` | validates UTF-8 and returns the text as a borrowed `&str` |

`from_ptr` is unsafe because none of its requirements can be checked: the
pointer may be null, dangling, unterminated, or point into memory that is
changing. It scans for the first zero byte to find the length, so an
unterminated buffer reads out of bounds. `len` and `to_str` are safe only
because `from_ptr` already promised a valid view.

`to_str` does not copy. The returned `&str` borrows the same bytes and keeps the
lifetime of the `&self` borrow of the `CStr` value; it is valid exactly as long as
that borrow. The C function that produced the buffer is responsible for keeping
it unchanged until the view is no longer used.

## Reading C text

The usual round trip creates the buffer with `CString` and reads it back through
`CStr`:

```ls
use std::cstring::CString;
use std::cstr::CStr;

fn main() {
    let owned = CString::new("lsaca").unwrap();
    let view = unsafe { CStr::from_ptr(owned.as_ptr()) };
    println!("{} {}", view.len(), view.to_str().unwrap());
}
```

Output:
```text
5 lsaca
```

The view borrows `owned` indirectly through a raw pointer, so the borrow checker
does not connect the two values. It is a mistake to drop, move or mutate
`owned` while `view` is still used, even though the compiler may accept the
code. Keep the owner alive in a local and finish all `CStr` reading before the
next mutation.

## Validity and text

`from_ptr` and `to_str` have separate jobs:

* `from_ptr` establishes the extent: the bytes are readable up to and including
  a NUL, and the memory is properly aligned for `u8`.
* `to_str` establishes the encoding: the bytes before the NUL are valid UTF-8
  and contain no interior NUL.

A C string may be arbitrary bytes; `to_str` returns `Err(Utf8Error)` for
invalid UTF-8. `len` still reports the byte count, because length is independent
of encoding. There is no unchecked text conversion and no lossy decoding in
2.5.0. The `Utf8Error` value describes the failure; it is not a panic.

`CStr` does not own its bytes, so dropping it does nothing. In particular it
never calls C `free`, never releases an ORC count and never removes a `CString`
owner. The buffer's owner must free it through its own path.

## Safety contract

`from_ptr` must be inside an unsafe context (`E0133`), and the pointer parameter
must be a raw pointer if it crosses an extern declaration (`E0770` checks the
declaration's types, not the call's validity). The programmer promises:

* the pointer is non-null and points to an allocation that stays alive for the
  whole borrow;
* a zero byte occurs within the allocation, so `len`/`to_str` terminate;
* the bytes are not mutated while the view is used;
* the pointer is not passed to a deallocator after the view is finished, and the
  view is not used after its owner is freed.

These promises are the pointer-lifetime and ORC rules in [Unsafe code and C
FFI](../language/32-unsafe-and-ffi.md#the-safety-contract). A dangling `CStr` is
the same hazard as any other dangling raw pointer: the compiler cannot diagnose
it, and sanitizer runs are the evidence used by the acceptance tests.

## See also

* [cstring](cstring.md) - the owning buffer that usually backs a `CStr`.
* [string](string.md) - the safe UTF-8 text type and its `&str` views.
* [ptr](ptr.md) and [mem](mem.md) - the rest of the unsafe-adjacent library
  surface.
* [Unsafe code and C FFI](../language/32-unsafe-and-ffi.md) - the complete
  safety contract.
