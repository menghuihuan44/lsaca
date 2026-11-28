# fs: the file system

**Status in 2.5.0.** Implemented in `stdlib/src/fs.ls` on top of the handle and
path primitives of [io](io.md). Every failure is an [`io::Error`](io.md), never
a panic. The v2.3 work adds `Path`, `PathBuf`, `read_dir_entries`, `DirEntry`
and `ReadDir`; the existing functions keep their `&str` path parameters.

Paths are text. The legacy functions take a read-only `&str`: pass a literal
directly, or `&owned` for a `String` variable. `Path` and `PathBuf` are owned
values that carry the same text and add lexical path operations; they do not
resolve the path or touch the file system unless an existence method is called.

## Functions

| Function | Signature | Notes |
| --- | --- | --- |
| `read_to_string` | `(path: &str) -> Result<String, Error>` | reads a whole UTF-8 file |
| `read` | `(path: &str) -> Result<Vec<u8>, Error>` | reads a whole file as bytes |
| `write` | `(path: &str, contents: &str) -> Result<(), Error>` | truncates or creates the file and writes text |
| `append` | `(path: &str, contents: &str) -> Result<(), Error>` | appends text; read-modify-write, not atomic |
| `copy` | `(from: &str, to: &str) -> Result<(), Error>` | copies a text file; read-modify-write, not atomic |
| `exists` | `(path: &str) -> bool` | whether the path exists |
| `is_file` | `(path: &str) -> bool` | whether the path is a regular file |
| `is_dir` | `(path: &str) -> bool` | whether the path is a directory |
| `metadata` | `(path: &str) -> Result<Metadata, Error>` | size, modification time and type |
| `read_dir` | `(path: &str) -> Result<Vec<String>, Error>` | sorted names, without `.` and `..` |
| `read_dir_entries` | `(path: &str) -> Result<ReadDir, Error>` | sorted directory entries with paths and file types |
| `create_dir` | `(path: &str) -> Result<(), Error>` | creates one directory |
| `create_dir_all` | `(path: &str) -> Result<(), Error>` | creates a directory and its missing parents |
| `remove_file` | `(path: &str) -> Result<(), Error>` | removes a file |
| `remove_dir` | `(path: &str) -> Result<(), Error>` | removes an empty directory |
| `remove_dir_all` | `(path: &str) -> Result<(), Error>` | removes a directory tree |
| `rename` | `(from: &str, to: &str) -> Result<(), Error>` | renames or moves a file or directory |
| `canonicalize` | `(path: &str) -> Result<String, Error>` | the absolute, resolved form of the path |
| `current_dir` | `() -> Result<String, Error>` | the working directory |
| `set_current_dir` | `(path: &str) -> Result<(), Error>` | changes the working directory |
| `temp_dir` | `() -> String` | the platform temporary directory |

`read_to_string` and `write` are text conveniences: they wrap the legacy
`read_file` / `write_file` helpers of [io](io.md), so a failure from them is an
`Error` of kind `ErrorKind::Other` carrying the operating system's message. The
other functions call classified primitives and report a precise kind through
`Error::last()`: a missing path is `ErrorKind::NotFound`, a permission failure is
`ErrorKind::PermissionDenied`, an existing target is `ErrorKind::AlreadyExists`,
and so on.

`append` and `copy` are defined in terms of a read followed by a write. They are
not atomic, they only handle UTF-8 text (binary files are not preserved), and a
concurrent writer can lose data. Use them for configuration and log-style files,
not as a general-purpose copy.

`read_dir` returns the names only, sorted, and never returns the `.` and `..`
entries. `read_dir_entries` returns the same sorted order but yields
`DirEntry` values. It reads the directory completely when the `ReadDir` value is
created; the value implements `Iterator<DirEntry>`, so it works with `for` and
the usual iterator adapters. A child path can be built without string
concatenation:

```rust
let mut child: PathBuf = PathBuf::from_str(path);
child.push(name);
```

## Paths

`Path` is a path value whose lexical operations never query the operating
system. `PathBuf` is the owned, mutable companion; it adds `push` and `pop`.
The two types are separate owned values, because standard-library structs cannot
store references: `PathBuf::as_path` copies the text into a `Path`.

```rust
pub struct Path {
    text: String,
}

pub struct PathBuf {
    text: String,
}
```

| `Path` item | Signature | Notes |
| --- | --- | --- |
| `Path::new` | `(text: &str) -> Path` | builds a path from text |
| `Path::from` | `(text: String) -> Path` | takes ownership of the text |
| `as_str` | `(&self) -> &str` | the path text |
| `to_string` | `(&self) -> String` | a copy of the path text |
| `display` | `(&self) -> String` | a copy for printing; there is no `Display` trait |
| `join` | `(&self, segment: &str) -> PathBuf` | appends a component; an absolute component replaces the path |
| `parent` | `(&self) -> Option<PathBuf>` | the parent path, or `None` at a root |
| `file_name` | `(&self) -> Option<String>` | the last component, or `None` at a root |
| `extension` | `(&self) -> Option<String>` | the text after the last dot of the file name, without the dot |
| `exists` | `(&self) -> bool` | whether the path exists |
| `is_file` | `(&self) -> bool` | whether the path is an existing regular file |
| `is_dir` | `(&self) -> bool` | whether the path is an existing directory |

`PathBuf` has the same lexical methods and the same existence checks, plus the
following:

| `PathBuf` item | Signature | Notes |
| --- | --- | --- |
| `PathBuf::new` | `() -> PathBuf` | an empty path |
| `PathBuf::from` | `(text: String) -> PathBuf` | takes ownership of the text |
| `PathBuf::from_str` | `(text: &str) -> PathBuf` | builds from text |
| `as_path` | `(&self) -> Path` | a `Path` with a copy of the text |
| `push` | `(&mut self, segment: &str)` | like `join`, storing the result |
| `pop` | `(&mut self) -> bool` | removes the last component; `false` at a root or empty path |

`join` and `push` insert a forward slash when they need a separator, on every
platform, so program output built from path text is stable across systems. An
empty segment leaves the path unchanged; an absolute segment replaces the whole
path. The platform decides which strings count as absolute and which separators
are recognised when splitting a path: Windows accepts both `/` and `\` and
understands drive-letter prefixes, while POSIX accepts only `/`.

`parent` returns `None` for an empty path and for a root such as `/` or a
Windows drive root. For a single relative component such as `"assets"` it
returns `Some("")`, so `pop` reports `true` and leaves an empty path. Trailing
separators are ignored when finding the parent, the file name and the extension.
`extension` returns `None` when the file name has no dot, when the dot is the
first character, or when the dot is the last character.

## Directory iteration

```rust
pub struct DirEntry {
    path: PathBuf,
    name: String,
    file_type: FileType,
}

pub struct ReadDir {
    entries: Vec<DirEntry>,
    index: usize,
}

pub fn read_dir_entries(path: &str) -> Result<ReadDir, Error>;
```

| Item | Signature | Notes |
| --- | --- | --- |
| `DirEntry::path` | `(&self) -> PathBuf` | the full path, built from the directory path and the entry name |
| `DirEntry::file_name` | `(&self) -> String` | the entry name without the directory prefix |
| `DirEntry::file_type` | `(&self) -> FileType` | regular file, directory or other |
| `ReadDir` `Iterator<DirEntry>` | `next(&mut self) -> Option<DirEntry>` | one entry per call, in sorted name order |

Each entry is classified with metadata when the `ReadDir` value is created. An
entry that disappears between the directory listing and that metadata check is
reported as `FileType::Other`; the iteration itself does not fail.

## Metadata and file types

```rust
pub struct Metadata {
    size: u64,
    modified_ms: i64,
    kind: i64,
}

pub enum FileType {
    File,
    Dir,
    Other,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `size` | `(&self) -> u64` | file size in bytes |
| `modified` | `(&self) -> Result<SystemTime, Error>` | modification time; an error when it is before the Unix epoch |
| `file_type` | `(&self) -> FileType` | regular file, directory or other |
| `modified_ms` | `(&self) -> i64` | deprecated in 2.1.0; use `modified` |
| `kind` | `(&self) -> i64` | deprecated in 2.1.0; use `file_type` |
| `is_file` | `(&self) -> bool` | shorthand for `file_type().is_file()` |
| `is_dir` | `(&self) -> bool` | shorthand for `file_type().is_dir()` |

`file_type()` returns the `FileType` enum added in 2.1.0; `FileType` has
`is_file()`, `is_dir()` and `is_other()`, all taking `self` because the type is
`Copy`. `size` returns `u64` and `modified` returns a
[`SystemTime`](time.md), matching the Rust shapes.

## Error classification

| Situation | Kind |
| --- | --- |
| missing file or directory | `NotFound` |
| access denied | `PermissionDenied` |
| `create_dir` of an existing directory | `AlreadyExists` |
| `remove_dir` of a non-empty directory, invalid path text | `InvalidInput` |
| failure inside `read_to_string` / `write` | `Other` |

## Cross-platform behaviour

* Path text uses the platform's syntax. The lexical `join` and `push`
  operations always insert `/`; Windows accepts both separators, POSIX only
  `/`.
* `Path` and `PathBuf` are lexical. They do not normalise `.` or `..`, do not
  resolve symbolic links and do not consult the file system except in
  `exists`, `is_file` and `is_dir`.
* `temp_dir` reads the platform temporary directory and falls back to `.`.
* `canonicalize` resolves symbolic links and returns an absolute path; on
  Windows the result may be an extended-length path.
* Modification times come from the operating system and may have coarse
  resolution (`modified_ms` exposes the raw millisecond count).

## Limits

There is no `OsStr` type, no symbolic-link API, no permission or ownership API,
no file watching and no recursive walker. `append` and `copy` are text-only and
not atomic. Reading a file that is not valid UTF-8 with `read_to_string` fails;
use `read` for bytes. Directory iteration is snapshot-based: the entries are
read when `read_dir_entries` is called, so changes made afterwards are not
reflected.

## See also

* [io](io.md) - file handles, errors and standard streams.
* [time](time.md) - `SystemTime` for modification times.
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
