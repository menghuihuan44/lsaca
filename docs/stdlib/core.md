# core: Option, Result and Iterator

**Status in 2.1.0.** `Option`, `Result` and `Iterator` live in `stdlib/src/core.ls`
(`std::core`) and are re-exported unqualified, so existing programs keep working.
The 2.1.0 additions are the `Option::or` and `Option::xor` combinators and the
`usize` counts in the iterator surface; the rest of the surface is unchanged.

## Option<T>

`Option<T>` is either `Some(value)` or `None`. It is a handle when `T` is a handle
and a plain value otherwise; it is `Copy` when `T` is `Copy`.

Methods defined in `stdlib/src/core.ls`:

| Method | Signature | Meaning |
| --- | --- | --- |
| `is_some` / `is_none` | `(&self) -> bool` | variant test |
| `unwrap` | `(self) -> T` | the value; panics on `None` |
| `unwrap_or` | `(self, T) -> T` | the value or the default |
| `expect` | `(self, &str) -> T` | the value; panics with the message on `None` |
| `or` | `(self, Option<T>) -> Option<T>` | the receiver when it is `Some`, otherwise the argument |
| `xor` | `(self, Option<T>) -> Option<T>` | `Some` only when exactly one side is `Some` |

The compiler also provides combinators by desugaring, so they are available
without being declared in the library:

| Combinator | Signature | Meaning |
| --- | --- | --- |
| `map` | `(Fn(T) -> U) -> Option<U>` | applies `f` to a `Some` value |
| `map_or` | `(U, Fn(T) -> U) -> U` | applies `f` or returns the default |
| `and_then` | `(Fn(T) -> Option<U>) -> Option<U>` | chains an `Option`-returning function |
| `filter` | `(Fn(&T) -> bool) -> Option<T>` | keeps `Some(v)` only when the predicate holds |
| `ok_or` | `(E) -> Result<T, E>` | converts to a `Result` |

Combinators may be chained; the closure argument receives the value by value except
for `filter`, which receives a reference. The full combinator walkthrough is in the
[Standard library chapter](../language/15-standard-library.md#option-combinators).

## Result<T, E>

`Result<T, E>` is either `Ok(value)` or `Err(error)`.

| Method | Signature | Meaning |
| --- | --- | --- |
| `is_ok` / `is_err` | `(&self) -> bool` | variant test |
| `unwrap` | `(self) -> T` | the value; panics on `Err` |
| `unwrap_or` | `(self, T) -> T` | the value or the default |
| `expect` | `(self, &str) -> T` | the value; panics with the message on `Err` |
| `ok` / `err` | `(self) -> Option<T>` / `Option<E>` | drops one side |
| `unwrap_err` | `(self) -> E` | the error; panics on `Ok` |

The compiler also desugars `map`, `map_err` and `and_then` on a `Result`
(`Fn(T) -> U`, `Fn(E) -> F` and `Fn(T) -> Result<U, E>` respectively).

The `?` operator propagates an `Err` from a function whose return type is a
compatible `Result`; it is a language feature documented in the
[Standard library chapter](../language/15-standard-library.md#error-propagation-with-).

## Error types

`ParseIntError` and `ParseFloatError` are defined here and both implement
`error::Error`; they are the error types of the typed `parse` methods (see
[string](string.md)).

## Iterator<T>

`Iterator<T>` is the trait every iterator implements:

```rust
pub trait Iterator<T> {
    fn next(&mut self) -> Option<T>;
}
```

It is a normal trait: user types can implement it, and generic code can bound on it
(see [Iterators](../language/19-iterators.md)). The adapters and terminal operations
live in [iter](iter.md).

## Planned for a later release

* Iterator fusion in the compiler (an optimisation, no API change).
* The remaining Rust `Option`/`Result` combinators (`or_else`, `unwrap_or_else`,
  `map_or_else`, `ok_or_else`, `unwrap_or_default`, `is_some_and`, `is_none_or`,
  `and`, `map_or`/`or`/`or_else`/`is_ok_and`/`is_err_and`/`inspect`/`inspect_err`
  on `Result`) are not part of 2.1.0.
* No `core`-level error trait: `std::error::Error` is defined in the error
  module and used by `io` and `fs`.

## See also

* [Standard library chapter](../language/15-standard-library.md) - full method
  reference and the `?` operator.
* [Iterators](iter.md) and [Collections](collections/README.md).
