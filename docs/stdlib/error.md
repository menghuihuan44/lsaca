# error: the Error trait

**Status in 2.1.0.** Implemented. The trait below is defined in
`stdlib/src/error.ls`; it is the whole module. Trait objects are available, so
`Box<dyn Error>` and `&dyn Error` can hold any implementation (see
[Trait objects](../language/25-trait-objects.md)).

```rust
pub trait Error {
    fn message(&self) -> String;
}
```

`Error` is the common shape for error values. It has one method and no associated
types, constants or generic methods, so it is object safe (see
[Trait objects](../language/25-trait-objects.md)): once a type implements it, a
`Box<dyn Error>` or a `&dyn Error` can hold any implementation. The standard
library implements it for `io::Error` (see [io](io.md)), `ParseIntError` and
`ParseFloatError` (see [string](string.md)).

User types may implement it too. The trait must be imported and named by its
single identifier: a path in an `impl` block is rejected with `E0001`
(`trait paths are not supported in impl blocks`), so write

```rust
use std::error::Error;

impl Error for ParseError {
    fn message(&self) -> String {
        format!("invalid number: {}", self.text)
    }
}
```

The trait is deliberately small. There is no `source`, `cause` or `backtrace`
method, no downcasting and no requirement that an error type implement `Display`
or `Debug`. Code that needs a category as well as a message should use a concrete
error type such as [`io::Error`](io.md), which carries an `ErrorKind` and a
message and works without dynamic dispatch.

## Limits in 2.1.0

The module is complete: one trait, one method. `io::Error` implements it, but
there is no blanket conversion between error types (`?>` and `From` are not part
of the language), so a function that propagates several error types still needs
one concrete error type. Error chaining and heterogeneous error vectors are not
part of 2.1.0.

## See also

* [io](io.md), [fs](fs.md) and [net](net.md) - the modules whose failures are
  reported through `io::Result`.
* [Trait objects](../language/25-trait-objects.md) - `Box<dyn Error>` and
  object safety.
