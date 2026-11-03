// File system functions.
//
// Paths are owned String values (string literals are String values); failures
// are reported as io::Error values with an ErrorKind classification, never as
// panics.
//
// Every function is a thin wrapper over the frozen Phase C intrinsic ABI
// (__io_*), which maps one to one onto the C runtime's ls_io_* functions.
pub extern fn read_file(path: String) -> Result<String, String>;
pub extern fn write_file(path: String, contents: String) -> Result<(), String>;

/// Reads a whole file as UTF-8 text.
pub fn read_to_string(path: String) -> Result<String, Error> {
    match read_file(path) {
        Ok(text) => {
            return Ok(text);
        }
        Err(message) => {
            return Err(Error::from_message(message));
        }
    }
}

/// Writes the whole contents of a file, creating or truncating it.
pub fn write(path: String, contents: String) -> Result<(), Error> {
    match write_file(path, contents) {
        Ok(unit) => {
            return Ok(unit);
        }
        Err(message) => {
            return Err(Error::from_message(message));
        }
    }
}

// ===========================================================================
// File system functions
// ===========================================================================

// Path level intrinsics (Appendix C).
pub extern fn __io_read_dir(path: String) -> Option<Vec<String>>;
pub extern fn __io_metadata(path: String) -> (i64, i64, i64, i64);
pub extern fn __io_remove_file(path: String) -> i64;
pub extern fn __io_remove_dir(path: String) -> i64;
pub extern fn __io_remove_dir_all(path: String) -> i64;
pub extern fn __io_create_dir(path: String) -> i64;
pub extern fn __io_create_dir_all(path: String) -> i64;
pub extern fn __io_rename(from: String, to: String) -> i64;
pub extern fn __io_canonicalize(path: String) -> Option<String>;
pub extern fn __io_current_dir() -> Option<String>;
pub extern fn __io_set_current_dir(path: String) -> i64;
pub extern fn __io_temp_dir() -> String;

/// File metadata read from the operating system.
///
/// kind: 0 for a regular file, 1 for a directory, 2 for anything else.
pub struct Metadata {
    size: i64,
    modified_ms: i64,
    kind: i64,
}

impl Metadata {
    /// The size of the file in bytes (0 for directories).
    pub fn size(&self) -> i64 {
        return self.size;
    }

    /// The last modification time in milliseconds since the Unix epoch.
    pub fn modified_ms(&self) -> i64 {
        return self.modified_ms;
    }

    /// The raw kind: 0 file, 1 directory, 2 other.
    pub fn kind(&self) -> i64 {
        return self.kind;
    }

    /// Whether the entry is a regular file.
    pub fn is_file(&self) -> bool {
        return self.kind == 0;
    }

    /// Whether the entry is a directory.
    pub fn is_dir(&self) -> bool {
        return self.kind == 1;
    }
}

/// Reads the entry names of a directory (not including . and ..), sorted.
pub fn read_dir(path: String) -> Result<Vec<String>, Error> {
    match __io_read_dir(path) {
        Some(entries) => {
            return Ok(entries);
        }
        None => {
            return Err(Error::last());
        }
    }
}

/// Reads metadata for a path.
pub fn metadata(path: String) -> Result<Metadata, Error> {
    let info = __io_metadata(path);
    if info.0 != 0 {
        return Err(Error::last());
    }
    return Ok(Metadata { size: info.1, modified_ms: info.2, kind: info.3 });
}

/// Whether the path exists (as a file, directory or other entry).
pub fn exists(path: String) -> bool {
    let info = __io_metadata(path);
    return info.0 == 0;
}

/// Whether the path is an existing regular file.
pub fn is_file(path: String) -> bool {
    let info = __io_metadata(path.clone());
    if info.0 != 0 {
        return false;
    }
    return info.3 == 0;
}

/// Whether the path is an existing directory.
pub fn is_dir(path: String) -> bool {
    let info = __io_metadata(path.clone());
    if info.0 != 0 {
        return false;
    }
    return info.3 == 1;
}

/// Appends text to a file, creating it when it does not exist yet.
///
/// The contents are read and rewritten as UTF-8 text in 2.0.0.
pub fn append(path: String, contents: String) -> Result<(), Error> {
    match read_to_string(path.clone()) {
        Ok(mut existing) => {
            existing.push_str(contents);
            return write(path, existing);
        }
        Err(error) => {
            return write(path, contents);
        }
    }
}

/// Copies a UTF-8 text file from one path to another.
pub fn copy(from: String, to: String) -> Result<(), Error> {
    match read_to_string(from) {
        Ok(text) => {
            return write(to, text);
        }
        Err(error) => {
            return Err(error);
        }
    }
}

/// Creates a directory.
pub fn create_dir(path: String) -> Result<(), Error> {
    let status: i64 = __io_create_dir(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Creates a directory and every missing parent directory.
pub fn create_dir_all(path: String) -> Result<(), Error> {
    let status: i64 = __io_create_dir_all(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Removes a file.
pub fn remove_file(path: String) -> Result<(), Error> {
    let status: i64 = __io_remove_file(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Removes an empty directory.
pub fn remove_dir(path: String) -> Result<(), Error> {
    let status: i64 = __io_remove_dir(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Removes a directory and everything below it.
pub fn remove_dir_all(path: String) -> Result<(), Error> {
    let status: i64 = __io_remove_dir_all(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Renames or moves a file or directory.
pub fn rename(from: String, to: String) -> Result<(), Error> {
    let status: i64 = __io_rename(from, to);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Resolves a path to its absolute, canonical form.
pub fn canonicalize(path: String) -> Result<String, Error> {
    match __io_canonicalize(path) {
        Some(resolved) => {
            return Ok(resolved);
        }
        None => {
            return Err(Error::last());
        }
    }
}

/// The current working directory.
pub fn current_dir() -> Result<String, Error> {
    match __io_current_dir() {
        Some(directory) => {
            return Ok(directory);
        }
        None => {
            return Err(Error::last());
        }
    }
}

/// Changes the current working directory.
pub fn set_current_dir(path: String) -> Result<(), Error> {
    let status: i64 = __io_set_current_dir(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// The operating system temporary directory.
pub fn temp_dir() -> String {
    return __io_temp_dir();
}


/// Reads a whole file as raw bytes.
///
/// The bytes are read in chunks through the handle intrinsics so the result
/// is a fresh Vec<u8> independent of the file afterwards.
pub fn read(path: String) -> Result<Vec<u8>, Error> {
    let mut file: File = File::open(path)?;
    let mut data: Vec<u8> = Vec::new();
    let mut chunk: Vec<u8> = Vec::new();
    while chunk.len() < 4096 {
        chunk.push(0);
    }
    loop {
        let count: i64 = file.read(chunk.as_mut_slice())?;
        if count == 0 {
            return Ok(data);
        }
        let mut index: i64 = 0;
        while index < count {
            data.push(chunk.get(index).unwrap());
            index = index + 1;
        }
    }
}
