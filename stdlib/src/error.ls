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

    /// Returns the underlying cause of this error, when there is one.
    ///
    /// The default implementation reports that there is no cause; error types
    /// that wrap another error can override it.
    fn source(&self) -> Option<Box<dyn Error>> {
        return None;
    }
}
