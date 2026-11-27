# io: input/output

**Status in 2.4.0.** Implemented. The error types, the file and standard-stream
handles, the text helpers and the handle-level primitives are implemented in
`stdlib/src/io.ls`. The v2.3 work adds the `Read`, `Write`, `Seek` and
`BufRead` traits, the `BufReader` and `BufWriter` adapters and
`File::read_to_string`. The `io::Result<T>` alias is not implemented.

Paths and text arguments are read-only `&str` slices, as everywhere in the
library: pass a literal directly, or `&owned` for a `String` variable. File
APIs keep taking `&str`; the owned `Path` and `PathBuf` values are documented
in [fs](fs.md). The legacy text helpers `read_file` and `write_file` still
report failures as a plain `String`, exactly as their 1.7 versions did; the
`fs` text wrappers built on them turn that message into an `io::Error` of kind
`ErrorKind::Other`.

## Text helpers

| Item | Signature | Notes |
| --- | --- | --- |
| `read_line` | `() -> Option<String>` | reads one line from standard input; `None` at end of input (1.7 helper) |
| `read_file` | `(path: &str) -> Result<String, String>` | reads a whole UTF-8 file; the error is the OS message (1.7 helper) |
| `write_file` | `(path: &str, contents: &str) -> Result<(), String>` | writes a whole text file; the error is the OS message (1.7 helper) |

`read_line` panics when the input is not valid UTF-8, as in 1.7. The
handle-level `BufRead::read_line` below reports invalid UTF-8 as an
`InvalidInput` error instead.

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

## Stream traits

The byte-stream traits are ordinary lsaca traits, so user types can implement
them and generic code can take bounds such as `R: Read`. `File` implements all
four traits; its inherent methods of the same names remain available and win
normal method dispatch.

| Trait | Methods | Notes |
| --- | --- | --- |
| `Read` | `read(&mut self, buffer: &mut [u8]) -> Result<usize, Error>` | reads up to `buffer.len()` bytes; `0` means end of stream |
| `Write` | `write(&mut self, buffer: &[u8]) -> Result<usize, Error>`, `flush(&mut self) -> Result<(), Error>`, default `write_all(&mut self, buffer: &[u8]) -> Result<(), Error>` | `write` may accept fewer bytes; `write_all` loops until the buffer is empty |
| `Seek` | `seek(&mut self, offset: i64, whence: i64) -> Result<u64, Error>` | `whence` 0 start, 1 current, 2 end; returns the new absolute position |
| `BufRead` | `read_line(&mut self, line: &mut String) -> Result<usize, Error>`, default `read_to_string(&mut self, buffer: &mut String) -> Result<usize, Error>` | UTF-8 text helpers; `read_line` appends the line, newline included |

`Write::write_all` reports an `ErrorKind::Other` error with the message
`failed to write whole buffer` when a write accepts zero bytes, instead of
looping forever. `BufRead::read_line` returns the number of bytes consumed,
including the trailing newline, and returns `0` at end of stream without
appending anything. `BufRead::read_to_string` repeatedly calls `read_line` and
returns the total number of bytes consumed.

Both text helpers validate UTF-8 before appending. Invalid bytes produce an
`ErrorKind::InvalidInput` error with the message
`stream did not contain valid UTF-8`; they are never replaced and the caller's
`String` is left unchanged. The line terminator is `\n`; a preceding `\r`
is preserved in the appended text.

The standard-library forwarding impls let the generic wrappers accept boxed
trait objects: `Box<dyn Read>` implements `Read`, and likewise for `Write`,
`Seek` and `BufRead`. This is what makes `BufReader<Box<dyn Read>>` and
`BufWriter<Box<dyn Write>>` work.

## Buffered adapters

`BufReader<R>` adds a byte buffer in front of any `R: Read`; `BufWriter<W>`
buffers bytes before sending them to any `W: Write`. The default capacity is
8192 bytes. Use them around a `File` or a boxed trait object:

| Type | Item | Signature | Notes |
| --- | --- | --- | --- |
| `BufReader<R>` | `new` | `(inner: R) -> BufReader<R>` | 8192-byte buffer |
| `BufReader<R>` | `with_capacity` | `(capacity: usize, inner: R) -> BufReader<R>` | explicit buffer size |
| `BufReader<R>` | `get_ref` | `(&self) -> &R` | the wrapped reader |
| `BufReader<R>` | `buffered` | `(&self) -> usize` | bytes buffered but not consumed |
| `BufWriter<W>` | `new` | `(inner: W) -> BufWriter<W>` | 8192-byte buffer |
| `BufWriter<W>` | `with_capacity` | `(capacity: usize, inner: W) -> BufWriter<W>` | explicit buffer size |
| `BufWriter<W>` | `get_ref` | `(&self) -> &W` | the wrapped writer |
| `BufWriter<W>` | `buffered` | `(&self) -> usize` | bytes waiting to be flushed |

`BufReader<R: Read>` implements `Read` and `BufRead`; its buffered range is
not exposed by `BufRead`, so `read_line` works through the same refill path as
`read`. `BufReader<R: Seek>` implements `Seek`: a relative seek is adjusted by
the number of buffered bytes and the buffer is discarded before the inner seek.
`BufWriter<W: Write>` implements `Write`; a write is accepted into the buffer
and triggers a flush of the buffer when it reaches the capacity.
`BufWriter<W: Write + Seek>` implements `Seek` by flushing first. A capacity of
zero makes every accepted byte flush immediately; a zero-capacity `BufReader`
passes each read straight to the inner reader.

`BufWriter` also flushes a non-empty buffer when it is dropped, ignoring any
error from that final flush. Call `flush` explicitly when the error matters.

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
| `read_to_string` | `(&mut self) -> Result<String, Error>` | reads the rest of the file as UTF-8 and advances to the end |
| `size` | `(&self) -> Result<u64, Error>` | file size in bytes |
| `set_nonblocking` | `(&self, enabled: bool) -> Result<(), Error>` | changes blocking mode where the platform supports it |
| `close` | `(&mut self) -> Result<(), Error>` | closes the handle; dropping afterwards is a no-op |
| `File` `Drop` | - | closes the handle unless it is a standard stream |

In 2.1.0 the byte counts and positions follow the Rust shapes: `read` and
`write` return the number of bytes transferred as `usize`, `seek` returns the
resulting position as `u64`, and `size` returns the file size as `u64`. The
`offset` and `whence` arguments stay `i64` because a seek offset may be
negative.

`File::read_to_string` validates the whole remaining file before returning it.
Invalid UTF-8 produces an `InvalidInput` error and no text; the bytes are not
silently replaced. The `BufRead` methods on `File` collect through the same
decoder, so a `File` can be read line by line without a `BufReader`; the
`BufReader` wrapper is still preferable for many small reads.

Buffers are slices, so `io` is where `&[u8]` and `&mut [u8]` appear in public
signatures (see [Slices and str](../language/26-slices-and-str.md)). A bare
`File` is unbuffered: every `read` and `write` is one operating-system call.
Wrap it in `BufReader` or `BufWriter` for buffering.

## Not implemented

The `io::Result<T>` alias is not implemented. There is no async I/O, no
memory-mapped file support and no socket type in `io` (TCP lives in
[net](net.md)). The forwarding impls cover `Box<dyn Trait>` only; `Rc<dyn Trait>`
and reference trait objects do not implement the stream traits.

## See also

* [fs](fs.md), [net](net.md) and [error](error.md).
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
