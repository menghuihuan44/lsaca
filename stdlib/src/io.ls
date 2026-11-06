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

use crate::error::Error as ErrorTrait;

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

impl Drop for File {
    fn drop(&mut self) {
        if self.close_on_drop {
            __io_close(self.handle);
        }
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
