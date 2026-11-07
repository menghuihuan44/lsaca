# fs: the file system

**Status in 2.2.0.** Implemented in `stdlib/src/fs.ls` on top of the handle and
path primitives of [io](io.md). Every path argument is a read-only `&str` slice
and every failure is an [`io::Error`](io.md), never a panic.

Paths are text: pass a literal directly, or `&owned` for a `String` variable.
The module has no `Path` or `PathBuf` type in 2.2.0; paths are platform strings
and the operating system decides what they mean.

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
entries. Build a child path with `String + &str` concatenation:

```rust
let mut dir: String = path;
dir += "/";
dir += name;
```

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

* Path separators are the platform's. There is no path-joining helper: build a
  path with `String + &str` or with the `+` operator on a `String`.
* `temp_dir` reads the platform temporary directory and falls back to `.`.
* `canonicalize` resolves symbolic links and returns an absolute path; on
  Windows the result may be an extended-length path.
* Modification times come from the operating system and may have coarse
  resolution (`modified_ms` exposes the raw millisecond count).

## Limits in 2.2.0

There is no `Path`, `PathBuf` or `OsStr` type, no directory-entry iterator, no
symbolic-link API, no permission or ownership API, no file watching and no
recursive walker. `append` and `copy` are text-only and not atomic. Reading a
file that is not valid UTF-8 with `read_to_string` fails; use `read` for bytes.

## See also

* [io](io.md) - file handles, errors and standard streams.
* [time](time.md) - `SystemTime` for modification times.
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
