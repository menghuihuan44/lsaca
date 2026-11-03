# io: input/output

**Status in 2.0.0.** Implemented. The error types, the file and standard-stream
handles and the text helpers are implemented in `stdlib/src/io.ls`. The `Read`,
`Write` and `BufRead` traits, `BufReader` and the `io::Result<T>` alias are not
implemented in 2.0.0 and are planned for a later release.

Paths are owned `String` values, as everywhere in the 2.0.0 library. The legacy
`read_file` / `write_file` helpers still return the failure as a `String`,
exactly as their 1.7 versions did.

## Text helpers

| Item | Signature | Notes |
| --- | --- | --- |
| `read_line` | `() -> Option<String>` | reads one line from standard input; `None` at end of input (1.7 helper) |
| `read_file` | `(String) -> Result<String, String>` | reads a whole UTF-8 file; the error is the OS message (1.7 helper) |
| `write_file` | `(String, String) -> Result<(), String>` | writes a whole text file; the error is the OS message (1.7 helper) |

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
    AddressInUse, AddressNotAvailable, BrokenPipe, Other,
}

pub struct Error {
    kind: ErrorKind,
    message: String,
}
```

| Method | Signature | Meaning |
| --- | --- | --- |
| `ErrorKind::from_id` | `(i64) -> ErrorKind` | maps the runtime's numeric kind; `0` and unknown values map to `Other` |
| `Error::kind` | `(&self) -> ErrorKind` | the error category |
| `Error::message` | `(&self) -> String` | the OS message, as text |
| `Error::from_message` | `(String) -> Error` | builds an error with kind `Other` from a text message |
| `Error::last` | `() -> Error` | builds an error from the runtime's most recent failure record |

The handle, path and socket intrinsics record a classified failure, so
`Error::last()` returns a precise kind for them: a missing file is
`ErrorKind::NotFound`, a refused connection is
`ErrorKind::ConnectionRefused`, and so on. The legacy `read_file` /
`write_file` helpers do not update that record; the `fs` text wrappers built on
them therefore report `ErrorKind::Other`.

## Handles

`stdin()`, `stdout()` and `stderr()` return the standard stream handles; `File`
is the result of opening or creating a file:

```rust
pub struct File {
    handle: i64,
    close_on_drop: bool,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `File::open` | `(String) -> Result<File, Error>` | opens an existing file for reading |
| `File::create` | `(String) -> Result<File, Error>` | creates or truncates a file for writing |
| `handle` | `(&self) -> i64` | the raw runtime table index |
| `read` | `(&mut self, &mut [u8]) -> Result<i64, Error>` | bytes read; `0` at end of file |
| `write` | `(&mut self, &[u8]) -> Result<i64, Error>` | bytes written |
| `seek` | `(&mut self, offset: i64, whence: i64) -> Result<i64, Error>` | moves the position; `whence` 0 start, 1 current, 2 end |
| `flush` | `(&mut self) -> Result<(), Error>` | pushes buffered data to the operating system |
| `size` | `(&self) -> Result<i64, Error>` | file size in bytes |
| `set_nonblocking` | `(&self, enabled: bool) -> Result<(), Error>` | changes blocking mode where the platform supports it |
| `close` | `(&mut self) -> Result<(), Error>` | closes the handle; dropping afterwards is a no-op |
| `File` `Drop` | - | closes the handle unless it is a standard stream |

Buffers are slices, so `io` is where `&[u8]` and `&mut [u8]` appear in public
signatures (see [Slices and str](../language/26-slices-and-str.md)). There is no
buffering layer: every `read` and `write` is one operating-system call.

## Planned for a later release

The `Read`, `Write` and `BufRead` traits, `BufReader` and the `io::Result<T>`
alias are not implemented in 2.0.0 and are planned for a later release. There is
no async I/O, no memory-mapped files and no socket
support in `io` (TCP lives in [net](net.md)).

## See also

* [fs](fs.md), [net](net.md) and [error](error.md).
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
