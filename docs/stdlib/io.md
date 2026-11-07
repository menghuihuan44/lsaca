# io: input/output

**Status in 2.2.0.** Implemented. The error types, the file and standard-stream
handles, the text helpers and the handle-level primitives are implemented in
`stdlib/src/io.ls`. The `Read`, `Write` and `BufRead` traits, `BufReader` and the
`io::Result<T>` alias are not implemented in 2.2.0 and are planned for a later
release.

Paths and text arguments are read-only `&str` slices, as everywhere in the 2.2.0
library: pass a literal directly, or `&owned` for a `String` variable. The legacy
text helpers `read_file` and `write_file` still report failures as a plain
`String`, exactly as their 1.7 versions did; the `fs` text wrappers built on them
turn that message into an `io::Error` of kind `ErrorKind::Other`.

## Text helpers

| Item | Signature | Notes |
| --- | --- | --- |
| `read_line` | `() -> Option<String>` | reads one line from standard input; `None` at end of input (1.7 helper) |
| `read_file` | `(path: &str) -> Result<String, String>` | reads a whole UTF-8 file; the error is the OS message (1.7 helper) |
| `write_file` | `(path: &str, contents: &str) -> Result<(), String>` | writes a whole text file; the error is the OS message (1.7 helper) |

`read_line` panics when the input is not valid UTF-8, as in 1.7.

## Errors

Operating-system failures never panic. The handle, path and socket operations
report an `Error` with a kind and a message; the legacy text helpers report a
plain `String`.

```rust
pub enum ErrorKind {
    NotFound, PermissionDenied, AlreadyExists, InvalidInput,
    ConnectionRefused, ConnectionReset, ConnectionAborted, TimedOut,
    WouldBlock, Interrupted, UnexpectedEof, Unsupported,
    AddrInUse, AddrNotAvailable, BrokenPipe, Other,
}

pub struct Error {
    kind: ErrorKind,
    message: String,
}
```

The two address variants were renamed in 2.1.0: `AddressInUse` and
`AddressNotAvailable` of 2.0.0 are now `AddrInUse` and `AddrNotAvailable`, the
Rust names. The variants map the runtime's numeric kinds 1 through 16 in order;
kind 0 (no error) and unknown values map to `ErrorKind::Other`.

| Method | Signature | Meaning |
| --- | --- | --- |
| `ErrorKind::from_id` | `(i64) -> ErrorKind` | maps the runtime's numeric kind |
| `Error::new` | `(ErrorKind, String) -> Error` | builds an error from a known classification and a message |
| `Error::other` | `(String) -> Error` | builds an error of kind `Other` from a message |
| `Error::kind` | `(&self) -> ErrorKind` | the error category |
| `Error::message` | `(&self) -> String` | the OS message, as text |
| `Error::from_message` | `(String) -> Error` | builds an `ErrorKind::Other` error when only text is known |
| `Error::last` | `() -> Error` | builds an error from the runtime's most recent failure record |

`io::Error` also implements the `std::error::Error` trait, so it can be used
behind a trait object (`Box<dyn error::Error>`); see [error](error.md).

The handle, path and socket operations record a classified failure, so
`Error::last()` returns a precise kind for them: a missing file is
`ErrorKind::NotFound`, a refused connection is
`ErrorKind::ConnectionRefused`, and so on. The legacy `read_file` /
`write_file` helpers do not update that record; the `fs` text wrappers built on
them therefore report `ErrorKind::Other`.

## Handles

`stdin()`, `stdout()` and `stderr()` return handles for the standard streams;
`File` is the result of opening or creating a file:

```rust
pub struct File {
    handle: i64,
    close_on_drop: bool,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `File::open` | `(path: &str) -> Result<File, Error>` | opens an existing file for reading |
| `File::create` | `(path: &str) -> Result<File, Error>` | creates or truncates a file for writing |
| `handle` | `(&self) -> i64` | the raw runtime table index |
| `read` | `(&mut self, buffer: &mut [u8]) -> Result<usize, Error>` | bytes read; `0` at end of file |
| `write` | `(&mut self, buffer: &[u8]) -> Result<usize, Error>` | bytes written |
| `seek` | `(&mut self, offset: i64, whence: i64) -> Result<u64, Error>` | moves the position and returns it; `whence` 0 start, 1 current, 2 end |
| `flush` | `(&mut self) -> Result<(), Error>` | pushes buffered data to the operating system |
| `size` | `(&self) -> Result<u64, Error>` | file size in bytes |
| `set_nonblocking` | `(&self, enabled: bool) -> Result<(), Error>` | changes blocking mode where the platform supports it |
| `close` | `(&mut self) -> Result<(), Error>` | closes the handle; dropping afterwards is a no-op |
| `File` `Drop` | - | closes the handle unless it is a standard stream |

In 2.1.0 the byte counts and positions follow the Rust shapes: `read` and
`write` return the number of bytes transferred as `usize`, `seek` returns the
resulting position as `u64`, and `size` returns the file size as `u64`. The
`offset` and `whence` arguments stay `i64` because a seek offset may be
negative.

Buffers are slices, so `io` is where `&[u8]` and `&mut [u8]` appear in public
signatures (see [Slices and str](../language/26-slices-and-str.md)). There is no
buffering layer: every `read` and `write` is one operating-system call.

## Planned for a later release

The `Read`, `Write` and `BufRead` traits, `BufReader` and the `io::Result<T>`
alias are not implemented in 2.2.0 and are planned for a later release. There is
no async I/O, no memory-mapped file support and no socket type in `io` (TCP
lives in [net](net.md)).

## See also

* [fs](fs.md), [net](net.md) and [error](error.md).
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
