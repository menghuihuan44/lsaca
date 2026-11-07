# error: the Error trait

**Status in 2.2.0.** Implemented. The trait below is defined in
`stdlib/src/error.ls`; it is the whole module. Trait objects are available, so
`Box<dyn Error>` and `&dyn Error` can hold any implementation (see
[Trait objects](../language/25-trait-objects.md)).

```rust
pub trait Error {
    fn message(&self) -> String;
    fn source(&self) -> Option<Box<dyn Error>> {
        return None;
    }
}
```

`Error` is the common shape for error values. `message` is required; `source` has a
default body that returns `None` and an implementation may override it to expose
the error that caused this one (see
[Error conversions with `?`](../language/30-from-try.md)). The trait has no
associated types, constants or generic methods, so it is still object safe (see
[Trait objects](../language/25-trait-objects.md)): once a type implements it, a
`Box<dyn Error>` or a `&dyn Error` can hold any implementation. The standard
library implements it for `io::Error` (see [io](io.md)), `ParseIntError` and
`ParseFloatError` (see [string](string.md)); those implementations keep the
default `source`.

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

The trait is deliberately small. There is no `cause` or `backtrace` method, no
downcasting, no blanket conversion into `Box<dyn Error>` and no requirement that
an error type implement `Display` or `Debug`. Code that needs a category as well
as a message should use a concrete error type such as [`io::Error`](io.md), which
carries an `ErrorKind` and a message and works without dynamic dispatch.

## Limits in 2.2.0

`io::Error`, `ParseIntError` and `ParseFloatError` implement the trait, and the
`?` operator converts an error through a user-written `impl From<...>` (see
[Error conversions with `?`](../language/30-from-try.md)). There is no blanket
conversion between error types, so a function that propagates several error types
still needs one concrete error type whose `From` implementations cover them.
Error chaining is manual (override `source` and walk it yourself) and
heterogeneous error vectors are not part of 2.2.0.

## See also

* [io](io.md), [fs](fs.md) and [net](net.md) - the modules whose failures are
  reported through `io::Result`.
* [Trait objects](../language/25-trait-objects.md) - `Box<dyn Error>` and
  object safety.
