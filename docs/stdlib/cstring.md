# cstring: CString

**Status.** Added in 2.5.0. `CString` owns a NUL-terminated byte buffer for
passing lsaca text to C. It is defined in `stdlib/src/cstring.ls` and re-exported
as `std::cstring::CString`; the supporting type `NulError` reports an embedded
NUL in the input.

`CString` is an ordinary managed value: it owns its buffer, is move-only,
participates in ORC reference counting and frees the buffer when the last handle
is dropped. It is **not** FFI-safe as a value; pass `as_ptr()` and a length, or a
raw pointer to the bytes, across the boundary. See [FFI-safe
types](../language/32-unsafe-and-ffi.md#ffi-safe-types).

## API

| Operation | Signature | Meaning |
| --- | --- | --- |
| `new` | `CString::new(text: &str) -> Result<CString, NulError>` | copies `text` and appends the terminating NUL; an embedded NUL is `Err` |
| `as_ptr` | `as_ptr(&self) -> *const u8` | pointer to the buffer, valid while the `CString` lives and is not moved or changed |
| `len` | `len(&self) -> usize` | byte length excluding the trailing NUL |
| `as_bytes` | `as_bytes(&self) -> &[u8]` | the bytes without the trailing NUL |
| `into_raw` | `into_raw(self) -> *mut u8` | gives up ownership and returns the buffer pointer; the caller must eventually return it with `from_raw` |
| `from_raw` | `unsafe CString::from_raw(ptr: *mut u8) -> CString` | takes ownership back; the pointer must come from `into_raw` and must not have been reclaimed |

`new` copies the text, so the original `&str` may be dropped immediately. The
buffer contains the UTF-8 bytes of the text followed by one zero byte. `len` on
the `CStr` view counts the bytes before that zero. The buffer is allocated by
lsaca's allocator, not by the C allocator.

`as_ptr` is a safe call, but the returned pointer is not safe to read after the
`CString` is dropped, moved or changed. A raw pointer does not keep the owner
alive. `into_raw` and `from_raw` move ownership through a raw pointer; between
the two calls the buffer is leaked by design and only the raw pointer refers to
it.

## Passing text to C

`strlen` is the smallest complete round trip: build a `CString`, pass its
pointer to the C function, and wrap the same pointer in a `CStr` to read it back.

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

The extern declaration must match the real C signature. `strlen` reads until the
NUL and does not retain the pointer; a C function that stores the pointer for
later use needs a separate lifetime agreement, because neither `CString` nor
`CStr` extends the C side's knowledge of when the buffer dies.

## Ownership and the C allocator

The allocator rule is the one place where `CString` differs from Rust:

* `CString::from_raw` accepts only pointers returned by `CString::into_raw`.
* A pointer from C `malloc`, `strdup` or a library function must **not** be
  passed to `from_raw`; the lsaca allocator would free memory it does not own.
* An lsaca `CString` buffer must not be passed to C `free`, and C code must not
  keep it after the `CString` is dropped.
* There is no C-allocator bridge in 2.5.0, because taking a free function would
  require function pointers.

`CString` is not a general byte-string type: `as_bytes` is a read-only view that
excludes the terminating NUL, and there is no way to mutate the buffer in place.
Use `CStr::from_ptr` and `CStr::to_str` for checked text, or keep the original
`String` alongside the C buffer.

## Safety contract

The compiler checks that `from_raw` is inside an unsafe context (`E0133`) and
that the pointer types in any extern declaration are FFI-safe (`E0770`). The
programmer promises that:

* the pointer came from `into_raw` and is returned exactly once;
* the buffer has not been freed or moved by another path;
* no other owner will free it after `from_raw` takes it back;
* C code does not write past the allocated `len + 1` bytes.

The allocator and pointer-validity promises are exactly the ORC and pointer
invariants in [Unsafe code and C FFI](../language/32-unsafe-and-ffi.md#orc-invariants).

## See also

* [cstr](cstr.md) - the borrowed NUL-terminated view.
* [ptr](ptr.md) and [mem](mem.md) - raw pointers and uninitialized storage.
* [Unsafe code and C FFI](../language/32-unsafe-and-ffi.md) - the safety
  contract and the FFI-safe type table.
* [string](string.md) - the owned UTF-8 text type used by `new`.
