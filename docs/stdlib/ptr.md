# ptr: raw pointer helpers

**Status.** Added in 2.5.0. This module provides the two null-pointer
constructors used by the unsafe layer; the pointer types themselves and their
operations are language syntax described in [Unsafe code and C
FFI](../language/32-unsafe-and-ffi.md#raw-pointers).

The module lives in `stdlib/src/ptr.ls`. Raw pointers are built into the
language, so the module has no types of its own; it only supplies the canonical
null values. The functions are generic over the pointee type, which must be
sized.

| Function | Signature | Meaning |
| --- | --- | --- |
| `null` | `null::<T>() -> *const T` | the null shared pointer, implemented as `0 as *const T` |
| `null_mut` | `null_mut::<T>() -> *mut T` | the null mutable pointer, implemented as `0 as *mut T` |

Both are ordinary safe functions: producing a null pointer is allowed. The
pointer becomes dangerous only when it is dereferenced, passed to pointer
arithmetic, or passed to C code that does not accept null. Check it with
`is_null()` before such a use.

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

## Pointer operations

The following methods belong to `*const T` and `*mut T`; the module page
lists them here because they are the complete helper surface:

| Method | Signature | Unsafe | Meaning |
| --- | --- | --- | --- |
| `is_null` | `(&self) -> bool` | no | true when the address is zero |
| `addr` | `(&self) -> usize` | no | the address as an integer |
| `add` | `(self, count: usize) -> *const T` / `*mut T` | yes | offset by `count` elements |
| `sub` | `(self, count: usize) -> *const T` / `*mut T` | yes | offset backwards by `count` elements |

`add` and `sub` keep the mutability of the receiver. The result must stay
inside the same allocation; the compiler does not check that. The `+` and
`-` operators are not defined for pointers.

```ls
fn main() {
    let value: i64 = 7;
    let raw = &value as *const i64;
    println!("{} {}", raw.is_null(), raw.addr() != 0);
}
```

Output:
```text
false true
```

## Creating pointers

The type checker accepts these forms:

* `&place as *const T` and `&mut place as *mut T`;
* `pointer as *const U` and `pointer as *mut U`;
* `pointer as usize`, and `n as *const T` or `n as *mut T`;
* the null constructors above.

A pointer cast changes only the static type, never the address. There is no
automatic conversion between references and pointers: the `as` cast is
required, and it is the only way to obtain a pointer from a reference.

## Safety and validity

A pointer is a plain `Copy` value with no lifetime, no ownership and no ORC
participation. The compiler does not keep its pointee alive, does not trace it
and does not know when it dangles. Dereferencing a pointer is unsafe and is
valid only while:

* the pointee still exists, is initialized and is properly aligned;
* no conflicting `&mut T` or write is live;
* the pointer has not been invalidated by a reallocation of its owner.

`String::as_ptr`, `Vec::as_ptr` and `Vec::as_mut_ptr` return pointers into a
managed buffer. Those pointers are valid only for the duration of the producing
borrow; mutating the owner may reallocate and invalidate them. A raw pointer
does not extend the borrow. See [Unsafe code and C
FFI](../language/32-unsafe-and-ffi.md#pointers-into-managed-buffers).

## See also

* [Unsafe code and C FFI](../language/32-unsafe-and-ffi.md) - the safety
  contract and ORC invariants.
* [cstring](cstring.md), [cstr](cstr.md) and [mem](mem.md) - the other 2.5.0
  unsafe-adjacent modules.
* [Types](../language/04-types.md#raw-pointers) - where pointer types fit in
  the type system.
