# From and ? conversion

Version 2.2 adds the `From` trait and uses it to convert error values
automatically when the `?` operator propagates a `Result`. The trait lives in
the standard library's `core` module and is re-exported from the std crate
root together with `Error`; the `Error` trait also gains a default
`source` method. Together these make a small error type that wraps lower-level
errors expressible without writing a `match` at every step.

## The From trait

```rust
pub trait From<T> {
    fn from(value: T) -> Self;
}
```

A type implements `From<Source>` to say that it can be built from a `Source`
value. Implementations are ordinary trait implementations:

```rust
struct ParseError {
    detail: String,
}

struct AppError {
    message: String,
}

impl From<ParseError> for AppError {
    fn from(value: ParseError) -> AppError {
        return AppError { message: value.detail };
    }
}
```

The standard library provides a few conversions of its own, for example
`impl From<String> for io::Error`, which wraps a message into an I/O error.
User code can implement `From` for its own error types and for standard types
where the language allows the implementation.

## How ? uses From

Before 2.2, `?` could only propagate an error into a function with the same error
type. Starting with 2.2, when the error types differ, the compiler looks for an
implementation of `From<FoundError>` for the function's `ExpectedError`. If one
exists, `expression?` is compiled as though written

```rust
match expression {
    Ok(value) => value,
    Err(error) => return Err(ExpectedError::from(error)),
}
```

so the conversion happens exactly once, at the `?`. If no `From`
implementation exists, the program is rejected with a type error about the
mismatched error types (`error[E0308]: mismatched types`) and a note
mentioning that the `?` needs a `From` conversion.

```rust
fn parse_port(text: &str) -> Result<i64, AppError> {
    let port = text.parse_i64()?;
    if port < 0 || port > 65535 {
        return Err(AppError { message: String::from("port out of range") });
    }
    return Ok(port);
}
```

Here `text.parse_i64()` has type `Result<i64, ParseIntError>` and
`parse_port` returns `Result<i64, AppError>`; the `?` succeeds because
`impl From<ParseIntError> for AppError` exists, and the returned error is
`AppError::from(error)`.

## The Error trait and source

The `Error` trait in 2.2 is:

```rust
pub trait Error {
    fn message(&self) -> String;

    fn source(&self) -> Option<Box<dyn Error>> {
        return None;
    }
}
```

`message` is unchanged from earlier versions; `source` is new and defaults
to `None`, so existing implementations keep compiling. An implementation that
wraps another error may override `source` to return it as a
`Box<dyn Error>`. The standard library implements `Error` for `io::Error` and
for the parse error types; `source` is used only when a program calls it,
because the language does not print error chains by itself.

## Combining From and a boxed error

A common pattern is to define one application error, convert the standard
library errors into it with `From`, and store the original as the `source`:

```rust
use std::error::Error;

struct AppError {
    message: String,
    cause: Option<Box<dyn Error>>,
}

impl From<ParseIntError> for AppError {
    fn from(value: ParseIntError) -> AppError {
        return AppError {
            message: value.message(),
            cause: Some(Box::new(value)),
        };
    }
}
```

## Known limitations in 2.2.0

* `?` applies to `Result` only; there is no `Option` conversion and no
  `Try` trait.
* There is no blanket `impl<T> From<T> for T` beyond the identity used by the
  checker, no `Into` trait, and no automatic conversion in `return` or
  assignment: `From` only changes what `?` does.
* `Error::source` is a default method; the language has no display or
  formatting protocol for errors, and error chains are never printed
  automatically.
* There is no `From<&str>` or `From<&String>` generic text conversion: text
  errors are converted through explicit `From<String>` implementations.
* Derive macros and automatic `Display`/`Debug` error implementations remain out
  of scope; a user error type writes its own `message` method.

## Summary

* `From<T>` is a standard-library trait with one required method,
  `fn from(value: T) -> Self`.
* `?` converts a `FoundError` into the function's `ExpectedError` when an
  `impl From<FoundError> for ExpectedError` exists, and otherwise reports a
  type error with a note about `From`.
* `Error` gained a `source` method with a default returning `None`; the standard
  error types implement `Error` and some `From` conversions.
* The conversion happens only through `?`; there is no general implicit
  conversion and no error-chain printing.

## What next

* [Error propagation with ?](15-standard-library.md#error-propagation-with-) covers
  the operator itself.
* [The error module](../stdlib/error.md) covers the `Error` trait and its
  implementations.
* [Traits](18-traits.md) covers the trait system that `From` uses.
* [Roadmap](16-roadmap.md) lists the remaining Rust features that 2.2.0 does
  not provide.
