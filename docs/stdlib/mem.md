# mem: MaybeUninit

**Status in 2.5.0.** Implemented in `stdlib/src/mem.ls` (`std::mem`).
`MaybeUninit<T>` is declared there as a marker struct,
`pub struct MaybeUninit<T> { internal: () }`, and the compiler gives that
wrapper exactly the size and alignment of `T`. Its methods are ordinary
`.ls` methods over a set of std-only `extern` intrinsics; `uninit`,
`write` and `as_mut_ptr` are safe, and unsafe `assume_init` consumes the
wrapper. It is the output slot for C functions that write through a pointer,
and the building block for values that are initialized in place.

`MaybeUninit<T>` does **not** add a tag, a flag or a hidden count. It is the same
bytes as `T`, and the compiler does not track whether those bytes are initialized.
The type is therefore a promise tool, not a safety net: `assume_init` trusts the
programmer, and reading storage that was never written is undefined behavior.

## API

| Operation | Signature | Unsafe | Meaning |
| --- | --- | --- | --- |
| `uninit` | `MaybeUninit::<T>::uninit() -> MaybeUninit<T>` | no | reserves storage; no value is initialized |
| `write` | `write(&mut self, value: T)` | no | writes `value` into the storage without reading the old bytes; returns nothing |
| `as_mut_ptr` | `as_mut_ptr(&mut self) -> *mut T` | no | pointer to the storage for C output parameters |
| `assume_init` | `assume_init(self) -> T` | yes | reads the storage as a `T`; the value must be initialized |

`write` replaces the bytes without dropping anything. That is the point: there
is no old value to drop. Calling `assume_init` produces a normal `T`, which is
then owned, moved and dropped like any other value. After `assume_init` the
`MaybeUninit` wrapper is consumed, so the same slot cannot be read twice through
that binding.

`as_mut_ptr` is safe because producing the pointer does not read the storage.
Passing it to C, or dereferencing it, still follows the ordinary raw-pointer
rules: the C function must write exactly one `T`, and the pointer must remain
valid for the call. Dereferencing it in lsaca requires an unsafe block and
`E0133` applies.

## Initializing in place

The normal pattern writes a value and then takes it out:

```ls
use std::mem::MaybeUninit;

fn main() {
    let mut slot = MaybeUninit::<i32>::uninit();
    slot.write(41);
    let value = unsafe { slot.assume_init() };
    println!("{}", value + 1);
}
```

Output:
```text
42
```

Because `write` is safe, only `assume_init` needs an unsafe block. The block is
the programmer's assertion that the storage really holds a `T`. If `write` were
skipped, or if a C function returned an error without writing, `assume_init`
would read uninitialized bytes and the behavior would be undefined.

`MaybeUninit` is most useful when a C function returns a status and fills an out
parameter:

```ls
use std::mem::MaybeUninit;

extern "C" {
    fn frexp(value: f64, exponent: *mut i32) -> f64;
}

fn main() {
    let mut exponent = MaybeUninit::<i32>::uninit();
    let fraction = unsafe { frexp(8.0, exponent.as_mut_ptr()) };
    let exponent = unsafe { exponent.assume_init() };
    println!("{} {}", fraction, exponent);
}
```

Output:
```text
0.5 4
```

`frexp` returns the fraction and writes the exponent through the pointer. The
unsafe block covers both the extern call and the read of the output slot; the
programmer must know from C that a successful call writes the exponent exactly
once. If the C function can fail without writing, the program must keep a
separate success flag and only call `assume_init` on the success path.

## Dropping and leaks

`MaybeUninit<T>` never runs `T`'s destructor. That is deliberate:

* An uninitialized `MaybeUninit` is dropped without touching `T`; there is no
  value to drop.
* A `MaybeUninit` holding an initialized value can be dropped without dropping
  that value. The value is then leaked, not double-freed.
* Calling `assume_init` transfers ownership to the returned `T`; that value's
  destructor runs normally, exactly once.

The compiler does not track which case a particular slot is in. A `MaybeUninit`
that is dropped after a successful `write` leaks the written value; this is
memory-safe but observable for types that own resources. Use `assume_init` on
every path that initialized the slot, or explicitly drop the value before
discarding the wrapper.

`MaybeUninit<T>` follows the Copy rule of `T`: when `T` is `Copy`, copying the
wrapper copies the bytes and neither copy drops anything. When `T` is not
`Copy`, the wrapper is not `Copy`. It is never traced by the ORC, never
refcounted and never receives retain or release calls, even when `T` contains
managed fields. The initialized `T` produced by `assume_init` is an ordinary
managed value again.

## FFI boundary

A `MaybeUninit<T>` itself is **not** FFI-safe and cannot appear in an extern
declaration (`E0770`). Pass `as_mut_ptr()`, which is a `*mut T`; the C side sees
a pointer to storage, not an lsaca wrapper. The pointed-to `T` must itself be
FFI-safe when C writes it: integers, floats, `bool`, `char`, raw pointers,
`repr(C)` structs or unions. A `MaybeUninit<String>` slot is legal lsaca code but
passing its pointer to C would let C write a managed value, which the safety
contract forbids. See [FFI-safe
types](../language/32-unsafe-and-ffi.md#ffi-safe-types).

## Safety contract

The compiler checks the unsafe context for `assume_init` (`E0133`) and the
FFI-safe types in extern signatures (`E0770`). It cannot check whether the bytes
are initialized. The programmer promises:

* `write` or a successful C call initialized the whole `T` exactly once;
* no second `assume_init` reads the same storage;
* the storage is not read while a C function is still writing it;
* if `T` owns resources, the initialized value is eventually dropped exactly
  once, or intentionally leaked with that consequence understood.

These are the initialization and ownership promises of [Unsafe code and C
FFI](../language/32-unsafe-and-ffi.md#the-safety-contract).

## See also

* [ptr](ptr.md) - raw pointers and pointer operations.
* [cstring](cstring.md) and [cstr](cstr.md) - owned and borrowed C text.
* [Unsafe code and C FFI](../language/32-unsafe-and-ffi.md) - the full safety
  contract and the out-parameter acceptance tests.
* [05-memory-model](../language/05-memory-model.md) - how ORC manages ordinary
  values, and why raw storage is outside it.
