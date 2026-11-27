// File system functions.
//
// Paths are read-only text parameters (a String variable is passed as
// &owned, and a string literal is passed without allocating). Failures are
// reported as io::Error values with an ErrorKind classification, never as
// panics.
//
// Every function is a thin wrapper over the frozen Phase C intrinsic ABI
// (__io_*), which maps one to one onto the C runtime's ls_io_* functions.
//
// Since 2.1.0 lengths are usize, sizes are u64, modified() returns the new
// time::SystemTime value and file_type() returns a FileType value.
pub extern fn read_file(path: &str) -> Result<String, String>;
pub extern fn write_file(path: &str, contents: &str) -> Result<(), String>;

/// Reads a whole file as UTF-8 text.
pub fn read_to_string(path: &str) -> Result<String, Error> {
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
pub fn write(path: &str, contents: &str) -> Result<(), Error> {
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
pub extern fn __io_read_dir(path: &str) -> Option<Vec<String>>;
pub extern fn __io_metadata(path: &str) -> (i64, i64, i64, i64);
pub extern fn __io_remove_file(path: &str) -> i64;
pub extern fn __io_remove_dir(path: &str) -> i64;
pub extern fn __io_remove_dir_all(path: &str) -> i64;
pub extern fn __io_create_dir(path: &str) -> i64;
pub extern fn __io_create_dir_all(path: &str) -> i64;
pub extern fn __io_rename(from: &str, to: &str) -> i64;
pub extern fn __io_canonicalize(path: &str) -> Option<String>;
pub extern fn __io_current_dir() -> Option<String>;
pub extern fn __io_set_current_dir(path: &str) -> i64;
pub extern fn __io_temp_dir() -> String;
pub extern fn __io_path_join(base: &str, segment: &str) -> String;
pub extern fn __io_path_parent(path: &str) -> Option<String>;
pub extern fn __io_path_file_name(path: &str) -> Option<String>;
pub extern fn __io_path_extension(path: &str) -> Option<String>;

/// The kind of a file system entry.
pub enum FileType {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// Anything else (a socket, a pipe or a device node).
    Other,
}

impl FileType {
    /// Whether the entry is a regular file.
    pub fn is_file(self) -> bool {
        match self {
            FileType::File => {
                return true;
            }
            _ => {
                return false;
            }
        }
    }

    /// Whether the entry is a directory.
    pub fn is_dir(self) -> bool {
        match self {
            FileType::Dir => {
                return true;
            }
            _ => {
                return false;
            }
        }
    }

    /// Whether the entry is neither a regular file nor a directory.
    pub fn is_other(self) -> bool {
        match self {
            FileType::Other => {
                return true;
            }
            _ => {
                return false;
            }
        }
    }
}

/// A borrowed-style path value with lexical operations.
///
/// Path never touches the operating system; join, parent, file_name and
/// extension work on the text alone. The checks exists, is_file and is_dir do
/// query the operating system. Path strings use the platform path syntax;
/// join always inserts a forward slash when it needs a separator.
pub struct Path {
    text: String,
}

impl Path {
    /// Builds a path from text.
    pub fn new(text: &str) -> Path {
        return Path { text: String::from_str(text) };
    }

    /// Builds a path from an owned string.
    pub fn from(text: String) -> Path {
        return Path { text: text };
    }

    /// The path text.
    pub fn as_str(&self) -> &str {
        return self.text.as_str();
    }

    /// The path as text.
    pub fn to_string(&self) -> String {
        return self.text.clone();
    }

    /// The path as text, suitable for printing.
    pub fn display(&self) -> String {
        return self.text.clone();
    }

    /// Joins a path component. An absolute component replaces this path.
    pub fn join(&self, segment: &str) -> PathBuf {
        return PathBuf { text: __io_path_join(self.text.as_str(), segment) };
    }

    /// The parent path, or None at a root.
    pub fn parent(&self) -> Option<PathBuf> {
        match __io_path_parent(self.text.as_str()) {
            Some(parent) => {
                return Some(PathBuf { text: parent });
            }
            None => {
                return None;
            }
        }
    }

    /// The last component of the path, or None at a root.
    pub fn file_name(&self) -> Option<String> {
        return __io_path_file_name(self.text.as_str());
    }

    /// The extension after the last dot of the file name, without the dot.
    pub fn extension(&self) -> Option<String> {
        return __io_path_extension(self.text.as_str());
    }

    /// Whether the path exists on the file system.
    pub fn exists(&self) -> bool {
        return exists(self.text.as_str());
    }

    /// Whether the path is an existing regular file.
    pub fn is_file(&self) -> bool {
        return is_file(self.text.as_str());
    }

    /// Whether the path is an existing directory.
    pub fn is_dir(&self) -> bool {
        return is_dir(self.text.as_str());
    }
}

/// An owned, mutable path value.
///
/// PathBuf adds new, push and pop to the lexical operations of Path. Path and
/// PathBuf are separate owned values: as_path clones the text rather than
/// borrowing it, because standard library structs cannot store references.
pub struct PathBuf {
    text: String,
}

impl PathBuf {
    /// Builds an empty path.
    pub fn new() -> PathBuf {
        return PathBuf { text: String::new() };
    }

    /// Builds a path from an owned string.
    pub fn from(text: String) -> PathBuf {
        return PathBuf { text: text };
    }

    /// Builds a path from text.
    pub fn from_str(text: &str) -> PathBuf {
        return PathBuf { text: String::from_str(text) };
    }

    /// The path text.
    pub fn as_str(&self) -> &str {
        return self.text.as_str();
    }

    /// A Path value with the same text.
    pub fn as_path(&self) -> Path {
        return Path { text: self.text.clone() };
    }

    /// The path as text.
    pub fn to_string(&self) -> String {
        return self.text.clone();
    }

    /// The path as text, suitable for printing.
    pub fn display(&self) -> String {
        return self.text.clone();
    }

    /// Appends a component. An absolute component replaces the path.
    pub fn push(&mut self, segment: &str) {
        let joined: String = __io_path_join(self.text.as_str(), segment);
        self.text = joined;
    }

    /// Removes the last component, returning false at a root or empty path.
    pub fn pop(&mut self) -> bool {
        match __io_path_parent(self.text.as_str()) {
            Some(parent) => {
                self.text = parent;
                return true;
            }
            None => {
                return false;
            }
        }
    }

    /// Joins a path component. An absolute component replaces this path.
    pub fn join(&self, segment: &str) -> PathBuf {
        return PathBuf { text: __io_path_join(self.text.as_str(), segment) };
    }

    /// The parent path, or None at a root.
    pub fn parent(&self) -> Option<PathBuf> {
        match __io_path_parent(self.text.as_str()) {
            Some(parent) => {
                return Some(PathBuf { text: parent });
            }
            None => {
                return None;
            }
        }
    }

    /// The last component of the path, or None at a root.
    pub fn file_name(&self) -> Option<String> {
        return __io_path_file_name(self.text.as_str());
    }

    /// The extension after the last dot of the file name, without the dot.
    pub fn extension(&self) -> Option<String> {
        return __io_path_extension(self.text.as_str());
    }

    /// Whether the path exists on the file system.
    pub fn exists(&self) -> bool {
        return exists(self.text.as_str());
    }

    /// Whether the path is an existing regular file.
    pub fn is_file(&self) -> bool {
        return is_file(self.text.as_str());
    }

    /// Whether the path is an existing directory.
    pub fn is_dir(&self) -> bool {
        return is_dir(self.text.as_str());
    }
}

/// File metadata read from the operating system.
///
/// kind: 0 for a regular file, 1 for a directory, 2 for anything else.
pub struct Metadata {
    size: u64,
    modified_ms: i64,
    kind: i64,
}

impl Metadata {
    /// The size of the file in bytes (0 for directories).
    pub fn size(&self) -> u64 {
        return self.size;
    }

    /// The last modification time as a SystemTime value.
    ///
    /// Modification times before the Unix epoch cannot be represented and are
    /// reported as an InvalidInput error.
    pub fn modified(&self) -> Result<SystemTime, Error> {
        if self.modified_ms < 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                String::from("file modification time is before the Unix epoch"),
            ));
        }
        return Ok(SystemTime::from_unix_ms(self.modified_ms));
    }

    /// The kind of the entry.
    pub fn file_type(&self) -> FileType {
        if self.kind == 0 {
            return FileType::File;
        }
        if self.kind == 1 {
            return FileType::Dir;
        }
        return FileType::Other;
    }

    /// The last modification time in milliseconds since the Unix epoch.
    ///
    /// Deprecated in 2.1.0: use modified() and the time types instead.
    pub fn modified_ms(&self) -> i64 {
        return self.modified_ms;
    }

    /// The raw kind: 0 file, 1 directory, 2 other.
    ///
    /// Deprecated in 2.1.0: use file_type() instead.
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
pub fn read_dir(path: &str) -> Result<Vec<String>, Error> {
    match __io_read_dir(path) {
        Some(entries) => {
            return Ok(entries);
        }
        None => {
            return Err(Error::last());
        }
    }
}

/// One entry returned while iterating a directory.
pub struct DirEntry {
    path: PathBuf,
    name: String,
    file_type: FileType,
}

impl DirEntry {
    /// The full path of the entry, built from the directory path and name.
    pub fn path(&self) -> PathBuf {
        return self.path.clone();
    }

    /// The entry name without the directory prefix.
    pub fn file_name(&self) -> String {
        return self.name.clone();
    }

    /// The kind of the entry, as reported by metadata at iteration time.
    pub fn file_type(&self) -> FileType {
        return self.file_type;
    }
}

/// A directory iterator over DirEntry values.
///
/// Entries are produced in sorted name order, matching read_dir, and the
/// directory is read completely when the value is created.
pub struct ReadDir {
    entries: Vec<DirEntry>,
    index: usize,
}

impl Iterator<DirEntry> for ReadDir {
    fn next(&mut self) -> Option<DirEntry> {
        if self.index >= self.entries.len() {
            return None;
        }
        let entry: DirEntry = self.entries.get(self.index).unwrap().clone();
        self.index = self.index + 1;
        return Some(entry);
    }
}

/// Reads the entries of a directory as DirEntry values.
///
/// Entry names are sorted, like read_dir. The file type of each entry is
/// classified from metadata when the iterator is created; an entry that
/// disappears during that check is reported as FileType::Other.
pub fn read_dir_entries(path: &str) -> Result<ReadDir, Error> {
    let names: Vec<String> = read_dir(path)?;
    let mut entries: Vec<DirEntry> = Vec::new();
    let mut index: usize = 0;
    while index < names.len() {
        let name: String = names.get(index).unwrap().clone();
        let mut child: PathBuf = PathBuf::from_str(path);
        child.push(name.as_str());
        let info = __io_metadata(child.as_str());
        let kind: i64 = if info.0 == 0 { info.3 } else { 2 };
        let file_type: FileType = if kind == 0 {
            FileType::File
        } else if kind == 1 {
            FileType::Dir
        } else {
            FileType::Other
        };
        entries.push(DirEntry { path: child, name: name, file_type: file_type });
        index = index + 1;
    }
    return Ok(ReadDir { entries: entries, index: 0 });
}

/// Reads metadata for a path.
pub fn metadata(path: &str) -> Result<Metadata, Error> {
    let info = __io_metadata(path);
    if info.0 != 0 {
        return Err(Error::last());
    }
    return Ok(Metadata { size: info.1 as u64, modified_ms: info.2, kind: info.3 });
}

/// Whether the path exists (as a file, directory or other entry).
pub fn exists(path: &str) -> bool {
    let info = __io_metadata(path);
    return info.0 == 0;
}

/// Whether the path is an existing regular file.
pub fn is_file(path: &str) -> bool {
    let info = __io_metadata(path);
    if info.0 != 0 {
        return false;
    }
    return info.3 == 0;
}

/// Whether the path is an existing directory.
pub fn is_dir(path: &str) -> bool {
    let info = __io_metadata(path);
    if info.0 != 0 {
        return false;
    }
    return info.3 == 1;
}

/// Appends text to a file, creating it when it does not exist yet.
///
/// The contents are read and rewritten as text: the file is created (or
/// truncated) when it does not exist yet.
pub fn append(path: &str, contents: &str) -> Result<(), Error> {
    match read_to_string(path) {
        Ok(mut existing) => {
            existing.push_str(contents);
            return write(path, &existing);
        }
        Err(error) => {
            return write(path, contents);
        }
    }
}

/// Copies a text file from one path to another.
pub fn copy(from: &str, to: &str) -> Result<(), Error> {
    match read_to_string(from) {
        Ok(text) => {
            return write(to, &text);
        }
        Err(error) => {
            return Err(error);
        }
    }
}

/// Creates a directory.
pub fn create_dir(path: &str) -> Result<(), Error> {
    let status: i64 = __io_create_dir(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Creates a directory and every missing parent directory.
pub fn create_dir_all(path: &str) -> Result<(), Error> {
    let status: i64 = __io_create_dir_all(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Removes a file.
pub fn remove_file(path: &str) -> Result<(), Error> {
    let status: i64 = __io_remove_file(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Removes an empty directory.
pub fn remove_dir(path: &str) -> Result<(), Error> {
    let status: i64 = __io_remove_dir(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Removes a directory and everything below it.
pub fn remove_dir_all(path: &str) -> Result<(), Error> {
    let status: i64 = __io_remove_dir_all(path);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Renames or moves a file or directory.
pub fn rename(from: &str, to: &str) -> Result<(), Error> {
    let status: i64 = __io_rename(from, to);
    if status < 0 {
        return Err(Error::last());
    }
    return Ok(());
}

/// Resolves a path to its absolute, canonical form.
pub fn canonicalize(path: &str) -> Result<String, Error> {
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
pub fn set_current_dir(path: &str) -> Result<(), Error> {
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
pub fn read(path: &str) -> Result<Vec<u8>, Error> {
    let mut file: File = File::open(path)?;
    let mut data: Vec<u8> = Vec::new();
    let mut chunk: Vec<u8> = Vec::new();
    while chunk.len() < 4096 {
        chunk.push(0);
    }
    loop {
        let count: usize = file.read(chunk.as_mut_slice())?;
        if count == 0 {
            return Ok(data);
        }
        let mut index: usize = 0;
        while index < count {
            data.push(chunk.get(index).unwrap());
            index = index + 1;
        }
    }
}
