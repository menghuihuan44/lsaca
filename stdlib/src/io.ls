// File, stream and error types.
//
// The operating system error classification is an enum so programs can react
// to failures without parsing message text. `Error::last()` builds an error
// from the runtime's most recent failure record; every fallible standard
// library operation reports failures this way instead of panicking.
//
// File values own a runtime handle and close it when dropped, except for the
// standard stream constructors, whose handles are never closed by the standard
// library. Handles are plain i64 runtime table indices.
//
// Since 2.1.0 read-only text parameters are `&str` (paths and text), byte
// counts and positions are `usize` (sizes stay `u64`), and `Error`
// implements the `error::Error` trait in addition to its inherent methods.

use crate::core::From;
use crate::error::Error as ErrorTrait;
use crate::string::__str_from_bytes;

// Intrinsic declarations for the runtime error record.
pub extern fn __io_last_error_kind() -> i64;
pub extern fn __io_last_error_message() -> String;

// Text input and output helpers.
//
// These are thin intrinsics: the compiler expands each call inline exactly as
// the legacy builtin table did. The structured io/fs modules build on top of
// them in a later phase. Their path and text arguments are `&str` slices.
pub extern fn read_line() -> Option<String>;
pub extern fn read_file(path: &str) -> Result<String, String>;
pub extern fn write_file(path: &str, contents: &str) -> Result<(), String>;

/// The classification of an operating system error.
///
/// The variants map the runtime's numeric kinds (1 through 16) in order; the
/// "no error" kind and unknown values map to `Other`.
pub enum ErrorKind {
    /// An entity was not found (missing file, directory or address).
    NotFound,
    /// The operation lacked permission to access the entity.
    PermissionDenied,
    /// The entity already exists (for example create on an existing file).
    AlreadyExists,
    /// The arguments were invalid for the operation.
    InvalidInput,
    /// No connection could be made because the target refused it.
    ConnectionRefused,
    /// An established connection was reset by the peer.
    ConnectionReset,
    /// A connection was aborted by the local end or the network.
    ConnectionAborted,
    /// The operation timed out.
    TimedOut,
    /// The operation would block (non blocking handles, pending accept).
    WouldBlock,
    /// The operation was interrupted and may be retried.
    Interrupted,
    /// The input ended before the requested amount of data was available.
    UnexpectedEof,
    /// The operation or type is not supported on this platform or handle.
    Unsupported,
    /// The requested address is already in use.
    AddrInUse,
    /// The requested address is not available on this host.
    AddrNotAvailable,
    /// A pipe or socket was closed while writing.
    BrokenPipe,
    /// Any other error.
    Other,
}

impl ErrorKind {
    /// Maps the numeric kind reported by the runtime to the enum.
    ///
    /// Kind `0` (no error) and unknown values map to `Other`.
    pub fn from_id(id: i64) -> ErrorKind {
        match id {
            1 => ErrorKind::NotFound,
            2 => ErrorKind::PermissionDenied,
            3 => ErrorKind::AlreadyExists,
            4 => ErrorKind::InvalidInput,
            5 => ErrorKind::ConnectionRefused,
            6 => ErrorKind::ConnectionReset,
            7 => ErrorKind::ConnectionAborted,
            8 => ErrorKind::TimedOut,
            9 => ErrorKind::WouldBlock,
            10 => ErrorKind::Interrupted,
            11 => ErrorKind::UnexpectedEof,
            12 => ErrorKind::Unsupported,
            13 => ErrorKind::AddrInUse,
            14 => ErrorKind::AddrNotAvailable,
            15 => ErrorKind::BrokenPipe,
            _ => ErrorKind::Other,
        }
    }
}

/// An operating system error with a classification and a message.
///
/// Values are obtained from `Error::last()` after a failed operation, or built
/// directly with `Error::new` / `Error::other`; the standard library never
/// panics on an operating system failure.
/// Building an `Error` from a plain message, so `?` can convert a text error
/// into an `io::Error` inside functions that return `Result<_, io::Error>`.
impl From<String> for Error {
    fn from(message: String) -> Error {
        return Error::other(message);
    }
}

pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    /// Builds an error from a classification and a message.
    pub fn new(kind: ErrorKind, message: String) -> Error {
        return Error { kind: kind, message: message };
    }

    /// Builds an error of kind `ErrorKind::Other` from a message.
    pub fn other(message: String) -> Error {
        return Error { kind: ErrorKind::Other, message: message };
    }

    /// The classification of this error.
    pub fn kind(&self) -> ErrorKind {
        return self.kind;
    }

    /// The human readable description of this error.
    pub fn message(&self) -> String {
        return self.message.clone();
    }

    /// Builds an error from a message when the runtime only reports text.
    ///
    /// The legacy content helpers report failures as strings; operations built
    /// on top of them classify those failures as ErrorKind::Other. Prefer
    /// `Error::new` when the classification is known.
    pub fn from_message(message: String) -> Error {
        return Error { kind: ErrorKind::Other, message: message };
    }
}

impl ErrorTrait for Error {
    fn message(&self) -> String {
        return self.message.clone();
    }
}

// ===========================================================================
// Stream traits
// ===========================================================================

/// A source of bytes.
///
/// A single call may read fewer bytes than the buffer can hold, including zero
/// at the end of the stream. Implementations never inspect the old contents of
/// the buffer.
pub trait Read {
    /// Reads up to buffer.len() bytes, returning the number read.
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error>;
}

/// A destination for bytes.
pub trait Write {
    /// Writes up to buffer.len() bytes, returning the number written.
    fn write(&mut self, buffer: &[u8]) -> Result<usize, Error>;

    /// Flushes any buffered bytes to the underlying destination.
    fn flush(&mut self) -> Result<(), Error>;

    /// Writes every byte of the buffer.
    ///
    /// A write that accepts zero bytes is reported as an error so the loop
    /// cannot spin forever.
    fn write_all(&mut self, buffer: &[u8]) -> Result<(), Error> {
        let mut remaining: Vec<u8> = __io_bytes_from(buffer, 0);
        while remaining.len() > 0 {
            let count: usize = self.write(remaining.as_slice())?;
            if count == 0 {
                return Err(Error::other(String::from("failed to write whole buffer")));
            }
            remaining = __io_bytes_from(remaining.as_slice(), count);
        }
        return Ok(());
    }
}

/// A changeable stream position.
///
/// whence is 0 for the start of the stream, 1 for the current position and 2
/// for the end; the returned value is the new absolute position.
pub trait Seek {
    /// Moves the stream position.
    fn seek(&mut self, offset: i64, whence: i64) -> Result<u64, Error>;
}

/// A buffered byte source that can produce UTF-8 lines and text.
pub trait BufRead {
    /// Reads one line and appends it to line, including the newline.
    ///
    /// Returns the number of bytes consumed, or zero at the end of the stream.
    fn read_line(&mut self, line: &mut String) -> Result<usize, Error>;

    /// Reads the rest of the stream and appends it to buffer.
    ///
    /// Returns the number of bytes consumed.
    fn read_to_string(&mut self, buffer: &mut String) -> Result<usize, Error> {
        let mut text: String = String::new();
        let mut total: usize = 0;
        loop {
            let count: usize = self.read_line(&mut text)?;
            if count == 0 {
                buffer.push_str(&text);
                return Ok(total);
            }
            total = total + count;
        }
    }
}

// ===========================================================================
// Text decoding helpers
// ===========================================================================

/// Copies bytes into a fresh Vec, starting at start.
fn __io_bytes_from(bytes: &[u8], start: usize) -> Vec<u8> {
    let mut copy: Vec<u8> = Vec::new();
    let mut index: usize = start;
    while index < bytes.len() {
        copy.push(bytes.get(index).unwrap());
        index = index + 1;
    }
    return copy;
}

fn __io_byte_between(value: i64, low: i64, high: i64) -> bool {
    return value >= low && value <= high;
}

fn __io_is_continuation(value: i64) -> bool {
    return __io_byte_between(value, 0x80, 0xBF);
}

/// Validates one UTF-8 sequence at a time.
///
/// Overlong forms, surrogate code points and values beyond U+10FFFF are
/// rejected, so a successful call always describes valid UTF-8.
fn __io_is_valid_utf8(bytes: &[u8]) -> bool {
    let length: usize = bytes.len();
    let mut index: usize = 0;
    while index < length {
        let first: i64 = bytes.get(index).unwrap() as i64;
        if first <= 0x7F {
            index = index + 1;
        } else if first >= 0xC2 && first <= 0xDF {
            if index + 1 >= length {
                return false;
            }
            let second: i64 = bytes.get(index + 1).unwrap() as i64;
            if !__io_is_continuation(second) {
                return false;
            }
            index = index + 2;
        } else if first >= 0xE0 && first <= 0xEF {
            if index + 2 >= length {
                return false;
            }
            let second: i64 = bytes.get(index + 1).unwrap() as i64;
            let third: i64 = bytes.get(index + 2).unwrap() as i64;
            if !__io_is_continuation(third) {
                return false;
            }
            if first == 0xE0 {
                if !__io_byte_between(second, 0xA0, 0xBF) {
                    return false;
                }
            } else if first == 0xED {
                if !__io_byte_between(second, 0x80, 0x9F) {
                    return false;
                }
            } else if !__io_is_continuation(second) {
                return false;
            }
            index = index + 3;
        } else if first >= 0xF0 && first <= 0xF4 {
            if index + 3 >= length {
                return false;
            }
            let second: i64 = bytes.get(index + 1).unwrap() as i64;
            let third: i64 = bytes.get(index + 2).unwrap() as i64;
            let fourth: i64 = bytes.get(index + 3).unwrap() as i64;
            if !__io_is_continuation(third) || !__io_is_continuation(fourth) {
                return false;
            }
            if first == 0xF0 {
                if !__io_byte_between(second, 0x90, 0xBF) {
                    return false;
                }
            } else if first == 0xF4 {
                if !__io_byte_between(second, 0x80, 0x8F) {
                    return false;
                }
            } else if !__io_is_continuation(second) {
                return false;
            }
            index = index + 4;
        } else {
            return false;
        }
    }
    return true;
}

/// Decodes a complete byte sequence as UTF-8 text.
fn __io_text_from_bytes(bytes: &[u8]) -> Result<String, Error> {
    if !__io_is_valid_utf8(bytes) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            String::from("stream did not contain valid UTF-8"),
        ));
    }
    return Ok(__str_from_bytes(bytes));
}

/// Reads one newline-terminated line through a byte source.
fn __io_read_line<R: Read>(reader: &mut R, line: &mut String) -> Result<usize, Error> {
    let mut data: Vec<u8> = Vec::new();
    let mut one: Vec<u8> = Vec::new();
    one.push(0);
    let mut total: usize = 0;
    loop {
        let count: usize = reader.read(one.as_mut_slice())?;
        if count == 0 {
            break;
        }
        total = total + count;
        let mut index: usize = 0;
        while index < count {
            data.push(one.get(index).unwrap());
            index = index + 1;
        }
        if data.get(data.len() - 1).unwrap() == 10 {
            break;
        }
    }
    let text: String = __io_text_from_bytes(data.as_slice())?;
    line.push_str(&text);
    return Ok(total);
}

/// Reads the rest of a byte source and appends it as UTF-8 text.
fn __io_read_to_string<R: Read>(reader: &mut R, buffer: &mut String) -> Result<usize, Error> {
    let mut data: Vec<u8> = Vec::new();
    let mut chunk: Vec<u8> = Vec::new();
    while chunk.len() < 4096 {
        chunk.push(0);
    }
    loop {
        let count: usize = reader.read(chunk.as_mut_slice())?;
        if count == 0 {
            break;
        }
        let mut index: usize = 0;
        while index < count {
            data.push(chunk.get(index).unwrap());
            index = index + 1;
        }
    }
    let text: String = __io_text_from_bytes(data.as_slice())?;
    buffer.push_str(&text);
    return Ok(data.len());
}

// ===========================================================================
// Handles and standard streams
// ===========================================================================

// Handle level intrinsics (Appendix C).
pub extern fn __io_stdin() -> i64;
pub extern fn __io_stdout() -> i64;
pub extern fn __io_stderr() -> i64;
pub extern fn __io_open(path: &str) -> i64;
pub extern fn __io_create(path: &str) -> i64;
pub extern fn __io_close(handle: i64) -> i64;
pub extern fn __io_read(handle: i64, buffer: &mut [u8]) -> i64;
pub extern fn __io_write(handle: i64, buffer: &[u8]) -> i64;
pub extern fn __io_seek(handle: i64, offset: i64, whence: i64) -> i64;
pub extern fn __io_flush(handle: i64) -> i64;
pub extern fn __io_size(handle: i64) -> i64;
pub extern fn __io_set_nonblocking(handle: i64, enabled: bool) -> i64;

impl Error {
    /// Builds an error from the runtime record of the last failed operation.
    pub fn last() -> Error {
        return Error {
            kind: ErrorKind::from_id(__io_last_error_kind()),
            message: __io_last_error_message(),
        };
    }
}

/// An open file or standard stream handle.
///
/// The handle is a runtime table index. File values own their handle and
/// close it when dropped, except for the standard stream constructors, whose
/// handles are never closed by the standard library. Operating system
/// failures are reported as Error values, never as panics.
pub struct File {
    handle: i64,
    close_on_drop: bool,
}

impl File {
    /// Opens an existing file for reading.
    pub fn open(path: &str) -> Result<File, Error> {
        let handle: i64 = __io_open(path);
        if handle < 0 {
            return Err(Error::last());
        }
        return Ok(File { handle: handle, close_on_drop: true });
    }

    /// Creates or truncates a file for writing.
    pub fn create(path: &str) -> Result<File, Error> {
        let handle: i64 = __io_create(path);
        if handle < 0 {
            return Err(Error::last());
        }
        return Ok(File { handle: handle, close_on_drop: true });
    }

    /// The raw runtime handle of this file.
    pub fn handle(&self) -> i64 {
        return self.handle;
    }

    /// Reads up to buffer.len() bytes, returning the number read.
    /// A return of 0 means the end of the file.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let read: i64 = __io_read(self.handle, buffer);
        if read < 0 {
            return Err(Error::last());
        }
        return Ok(read as usize);
    }

    /// Writes the whole buffer, returning the number of bytes written.
    pub fn write(&mut self, buffer: &[u8]) -> Result<usize, Error> {
        let written: i64 = __io_write(self.handle, buffer);
        if written < 0 {
            return Err(Error::last());
        }
        return Ok(written as usize);
    }

    /// Moves the file position. whence: 0 start, 1 current, 2 end.
    ///
    /// A negative result from the runtime is reported as an Error; the runtime
    /// classifies a negative resulting position as InvalidInput.
    pub fn seek(&mut self, offset: i64, whence: i64) -> Result<u64, Error> {
        let position: i64 = __io_seek(self.handle, offset, whence);
        if position < 0 {
            return Err(Error::last());
        }
        return Ok(position as u64);
    }

    /// Flushes buffered data to the operating system.
    pub fn flush(&mut self) -> Result<(), Error> {
        let status: i64 = __io_flush(self.handle);
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(());
    }

    /// Reads the rest of the file and returns it as UTF-8 text.
    ///
    /// Invalid UTF-8 is reported as an InvalidInput error; the bytes are not
    /// silently replaced. The file position advances to the end.
    pub fn read_to_string(&mut self) -> Result<String, Error> {
        let mut text: String = String::new();
        __io_read_to_string(self, &mut text)?;
        return Ok(text);
    }

    /// The total size of the file in bytes.
    ///
    /// Sizes are u64 (like fs::Metadata::size); byte counts and positions are
    /// usize.
    pub fn size(&self) -> Result<u64, Error> {
        let size: i64 = __io_size(self.handle);
        if size < 0 {
            return Err(Error::last());
        }
        return Ok(size as u64);
    }

    /// Enables or disables non blocking mode where the platform supports it.
    pub fn set_nonblocking(&self, enabled: bool) -> Result<(), Error> {
        let status: i64 = __io_set_nonblocking(self.handle, enabled);
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(());
    }

    /// Closes the file. Dropping the value afterwards is a no-op.
    pub fn close(&mut self) -> Result<(), Error> {
        if !self.close_on_drop {
            return Ok(());
        }
        let status: i64 = __io_close(self.handle);
        self.close_on_drop = false;
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(());
    }
}

impl Read for File {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        return self.read(buffer);
    }
}

impl Write for File {
    fn write(&mut self, buffer: &[u8]) -> Result<usize, Error> {
        return self.write(buffer);
    }

    fn flush(&mut self) -> Result<(), Error> {
        return self.flush();
    }
}

impl Seek for File {
    fn seek(&mut self, offset: i64, whence: i64) -> Result<u64, Error> {
        return self.seek(offset, whence);
    }
}

impl BufRead for File {
    fn read_line(&mut self, line: &mut String) -> Result<usize, Error> {
        return __io_read_line(self, line);
    }

    fn read_to_string(&mut self, buffer: &mut String) -> Result<usize, Error> {
        return __io_read_to_string(self, buffer);
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if self.close_on_drop {
            __io_close(self.handle);
        }
    }
}

// Internal bridge helpers. Calling a trait method directly on a struct field
// would borrow the whole receiver in the current borrow checker, so these free
// functions borrow the inner value as an ordinary local parameter instead.

fn __io_fill<R: Read>(inner: &mut R, buffer: &mut [u8]) -> Result<usize, Error> {
    return inner.read(buffer);
}

fn __io_send_all<W: Write>(inner: &mut W, bytes: &[u8]) -> Result<(), Error> {
    return inner.write_all(bytes);
}

fn __io_finish<W: Write>(inner: &mut W) -> Result<(), Error> {
    return inner.flush();
}

// ===========================================================================
// Buffered adapters
// ===========================================================================

/// A reader that buffers byte reads from an inner reader.
///
/// The default capacity is 8192 bytes. BufReader implements Read and BufRead;
/// pass it to code that expects an R: Read bound, or read UTF-8 lines with
/// read_line.
pub struct BufReader<R> {
    inner: R,
    buffer: Vec<u8>,
    position: usize,
    filled: usize,
}

impl<R> BufReader<R> {
    /// Wraps a reader with the default 8192 byte buffer.
    pub fn new(inner: R) -> BufReader<R> {
        return BufReader::with_capacity(8192, inner);
    }

    /// Wraps a reader with an explicit buffer capacity.
    pub fn with_capacity(capacity: usize, inner: R) -> BufReader<R> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut index: usize = 0;
        while index < capacity {
            buffer.push(0);
            index = index + 1;
        }
        return BufReader {
            inner: inner,
            buffer: buffer,
            position: 0,
            filled: 0,
        };
    }

    /// A reference to the wrapped reader.
    pub fn get_ref(&self) -> &R {
        return &self.inner;
    }

    /// The number of buffered bytes that have not been consumed yet.
    pub fn buffered(&self) -> usize {
        return self.filled - self.position;
    }
}

impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let wanted: usize = buffer.len();
        if wanted == 0 {
            return Ok(0);
        }
        if self.buffer.len() == 0 {
            return __io_fill(&mut self.inner, buffer);
        }
        if self.position >= self.filled {
            let mut temp: Vec<u8> = Vec::new();
            let mut prefill: usize = 0;
            while prefill < self.buffer.len() {
                temp.push(0);
                prefill = prefill + 1;
            }
            let count: usize = __io_fill(&mut self.inner, temp.as_mut_slice())?;
            if count == 0 {
                return Ok(0);
            }
            let mut copy: usize = 0;
            while copy < count {
                self.buffer[copy] = temp.get(copy).unwrap();
                copy = copy + 1;
            }
            self.position = 0;
            self.filled = count;
        }
        let available: usize = self.filled - self.position;
        let count: usize = if wanted < available { wanted } else { available };
        let mut index: usize = 0;
        while index < count {
            buffer[index] = self.buffer.get(self.position + index).unwrap();
            index = index + 1;
        }
        self.position = self.position + count;
        return Ok(count);
    }
}

impl<R: Read> BufRead for BufReader<R> {
    fn read_line(&mut self, line: &mut String) -> Result<usize, Error> {
        return __io_read_line(self, line);
    }

    fn read_to_string(&mut self, buffer: &mut String) -> Result<usize, Error> {
        return __io_read_to_string(self, buffer);
    }
}

impl<R: Seek> Seek for BufReader<R> {
    fn seek(&mut self, offset: i64, whence: i64) -> Result<u64, Error> {
        let mut adjusted: i64 = offset;
        if whence == 1 {
            adjusted = adjusted - ((self.filled - self.position) as i64);
        }
        self.position = 0;
        self.filled = 0;
        return self.inner.seek(adjusted, whence);
    }
}

/// A writer that buffers byte writes before sending them to an inner writer.
///
/// The default capacity is 8192 bytes. Writes smaller than the remaining
/// capacity are accepted immediately; reaching the capacity flushes the buffer
/// to the inner writer. Call flush before dropping with a capacity of zero.
pub struct BufWriter<W> {
    inner: W,
    buffer: Vec<u8>,
    capacity: usize,
}

impl<W> BufWriter<W> {
    /// Wraps a writer with the default 8192 byte buffer.
    pub fn new(inner: W) -> BufWriter<W> {
        return BufWriter::with_capacity(8192, inner);
    }

    /// Wraps a writer with an explicit buffer capacity.
    pub fn with_capacity(capacity: usize, inner: W) -> BufWriter<W> {
        return BufWriter {
            inner: inner,
            buffer: Vec::new(),
            capacity: capacity,
        };
    }

    /// A reference to the wrapped writer.
    pub fn get_ref(&self) -> &W {
        return &self.inner;
    }

    /// The number of bytes waiting in the buffer.
    pub fn buffered(&self) -> usize {
        return self.buffer.len();
    }
}

impl<W: Write> BufWriter<W> {
    fn flush_buffer(&mut self) -> Result<(), Error> {
        if self.buffer.len() > 0 {
            let mut pending: Vec<u8> = Vec::new();
            let mut index: usize = 0;
            while index < self.buffer.len() {
                pending.push(self.buffer.get(index).unwrap());
                index = index + 1;
            }
            self.buffer.clear();
            let result = __io_send_all(&mut self.inner, pending.as_slice());
            match result {
                Ok(()) => {}
                Err(error) => {
                    return Err(error);
                }
            }
        }
        let result = __io_finish(&mut self.inner);
        return result;
    }
}

impl<W: Write> Write for BufWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> Result<usize, Error> {
        let mut index: usize = 0;
        while index < buffer.len() {
            self.buffer.push(buffer.get(index).unwrap());
            index = index + 1;
            if self.capacity == 0 || self.buffer.len() >= self.capacity {
                self.flush_buffer()?;
            }
        }
        return Ok(index);
    }

    fn flush(&mut self) -> Result<(), Error> {
        return self.flush_buffer();
    }
}

impl<W: Write> Drop for BufWriter<W> {
    fn drop(&mut self) {
        if self.buffer.len() > 0 {
            let _ = self.flush_buffer();
        }
    }
}

impl<W: Write + Seek> Seek for BufWriter<W> {
    fn seek(&mut self, offset: i64, whence: i64) -> Result<u64, Error> {
        self.flush_buffer()?;
        return self.inner.seek(offset, whence);
    }
}

// Trait objects can be wrapped by generic readers and writers only when the
// trait is also implemented for Box<dyn Trait>. These forwarding impls give
// the wrappers that bound while calls still dispatch to the boxed value.
impl Read for Box<dyn Read> {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        return self.read(buffer);
    }
}

impl Write for Box<dyn Write> {
    fn write(&mut self, buffer: &[u8]) -> Result<usize, Error> {
        return self.write(buffer);
    }

    fn flush(&mut self) -> Result<(), Error> {
        return self.flush();
    }
}

impl Seek for Box<dyn Seek> {
    fn seek(&mut self, offset: i64, whence: i64) -> Result<u64, Error> {
        return self.seek(offset, whence);
    }
}

impl BufRead for Box<dyn BufRead> {
    fn read_line(&mut self, line: &mut String) -> Result<usize, Error> {
        return self.read_line(line);
    }

    fn read_to_string(&mut self, buffer: &mut String) -> Result<usize, Error> {
        return self.read_to_string(buffer);
    }
}

/// The standard input stream.
pub fn stdin() -> File {
    return File { handle: __io_stdin(), close_on_drop: false };
}

/// The standard output stream.
pub fn stdout() -> File {
    return File { handle: __io_stdout(), close_on_drop: false };
}

/// The standard error stream.
pub fn stderr() -> File {
    return File { handle: __io_stderr(), close_on_drop: false };
}
