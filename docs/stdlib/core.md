# core: Option, Result and Iterator

**Status in 2.3.0.** `Option`, `Result` and `Iterator` live in `stdlib/src/core.ls`
(`std::core`) and are re-exported unqualified, so existing programs keep working.
The 2.1.0 additions are the `Option::or` and `Option::xor` combinators and the
`usize` counts in the iterator surface; 2.2.0 adds the `From` trait used by the
`?` operator, defined here and re-exported by the crate root.

Version 2.3 adds the remaining Rust `Option` and `Result` combinators listed in
deviation 12 of the roadmap; `Option::unwrap_or_default` waits for the `Default`
trait planned for v2.4.

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
| `map_or_else` | `(Fn() -> U, Fn(T) -> U) -> U` | calls the mapping closure for `Some` and the default closure for `None` |
| `and_then` | `(Fn(T) -> Option<U>) -> Option<U>` | chains an `Option`-returning function |
| `or_else` | `(Fn() -> Option<T>) -> Option<T>` | the receiver when `Some`, otherwise calls the closure |
| `filter` | `(Fn(&T) -> bool) -> Option<T>` | keeps `Some(v)` only when the predicate holds |
| `ok_or` | `(E) -> Result<T, E>` | converts to a `Result` |
| `ok_or_else` | `(Fn() -> E) -> Result<T, E>` | converts to a `Result`, building the error lazily |
| `unwrap_or_else` | `(Fn() -> T) -> T` | the value or the lazy default |
| `is_some_and` | `(Fn(T) -> bool) -> bool` | `true` only for `Some(v)` where the predicate holds |
| `is_none_or` | `(Fn(T) -> bool) -> bool` | `true` for `None` or `Some(v)` where the predicate holds |
| `and` | `(Option<U>) -> Option<U>` | the argument when both are `Some`, otherwise `None` |

Combinators may be chained; closure arguments receive the value by value except
for `filter`, which receives a reference. The eager arguments of `or`, `and`,
`map_or` and `ok_or` are evaluated before the match; the `*_else` forms call
their closure only for the relevant variant. The full combinator walkthrough is in
the [Standard library chapter](../language/15-standard-library.md#option-combinators).

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

The compiler also desugars the following `Result` combinators:

| Combinator | Signature | Meaning |
| --- | --- | --- |
| `map` | `(Fn(T) -> U) -> Result<U, E>` | applies `f` to an `Ok` value |
| `map_or` | `(U, Fn(T) -> U) -> U` | applies `f` or returns the default |
| `map_or_else` | `(Fn(E) -> U, Fn(T) -> U) -> U` | calls the mapping closure for `Ok` and the error closure for `Err` |
| `map_err` | `(Fn(E) -> F) -> Result<T, F>` | applies `f` to an `Err` value |
| `and_then` | `(Fn(T) -> Result<U, E>) -> Result<U, E>` | chains a `Result`-returning function |
| `or` | `(Result<T, F>) -> Result<T, F>` | the receiver when `Ok`, otherwise the argument |
| `or_else` | `(Fn(E) -> Result<T, F>) -> Result<T, F>` | the receiver when `Ok`, otherwise calls the closure |
| `is_ok_and` | `(Fn(T) -> bool) -> bool` | `true` only for `Ok(v)` where the predicate holds |
| `is_err_and` | `(Fn(E) -> bool) -> bool` | `true` only for `Err(e)` where the predicate holds |
| `inspect` | `(Fn(&T) -> ()) -> Result<T, E>` | borrows the `Ok` value for a side effect |
| `inspect_err` | `(Fn(&E) -> ()) -> Result<T, E>` | borrows the `Err` value for a side effect |
| `unwrap_or_else` | `(Fn(E) -> T) -> T` | the value or the lazy default |

The `?` operator propagates an `Err` from a function whose return type is a
compatible `Result`; it is a language feature documented in the
[Standard library chapter](../language/15-standard-library.md#error-propagation-with-).

## Error types

`ParseIntError` and `ParseFloatError` are defined here and both implement
`error::Error`; they are the error types of the typed `parse` methods (see
[string](string.md)).

## The From trait

The library defines the conversion trait behind the `?` operator:

`@rust
pub trait From<T> {
    fn from(value: T) -> Self;
}
`@

It lives in `stdlib/src/core.ls` and the crate root re-exports it, so `From`,
`SomeType::from(value)` and `impl From<...> for ...` work without an import.
A user error type converts with

`@rust
impl From<ParseIntError> for AppError {
    fn from(error: ParseIntError) -> AppError {
        return AppError { message: error.message() };
    }
}
`@

`?` calls `From::from` only when the `Err` type of the expression differs from
the `Err` type of the enclosing function; the full mechanism is in
[Error conversions with `?`](../language/30-from-try.md). There is no blanket
`impl<T> From<T> for T`, no `Into`, no `Try` and no `From<&str>` conversions:
only the pairs the library or the user writes exist.

## Iterator<T>

`Iterator<T>` is the trait every iterator implements:

`@rust
pub trait Iterator<T> {
    fn next(&mut self) -> Option<T>;
}
`@

It is a normal trait: user types can implement it, and generic code can bound on it
(see [Iterators](../language/19-iterators.md)). The adapters and terminal operations
live in [iter](iter.md).

## Planned for a later release

* Iterator fusion in the compiler (an optimisation, no API change).
* `Option::unwrap_or_default` waits for the `Default` trait; protocol work
  (including `Default`) is planned for v2.4.
* No `core`-level error trait: `std::error::Error` is defined in the error
  module and used by `io` and `fs`.

## See also

* [Standard library chapter](../language/15-standard-library.md) - full method
  reference and the `?` operator.
* [Iterators](iter.md) and [Collections](collections/README.md).
