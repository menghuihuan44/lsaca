// Standard library error trait.
//
// `Error` is implemented by error values that can describe themselves as
// human readable text. The concrete error type used by the `io`, `fs`, `net`
// and `time` modules is `io::Error`, which carries an `ErrorKind`
// classification plus a message.

/// A value that can describe itself as human readable text.
pub trait Error {
    /// Returns the human readable description of this error.
    fn message(&self) -> String;
}
