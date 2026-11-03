# fs: files and directories

**Status in 2.0.0.** Implemented. The path, directory and metadata API below is
implemented in `stdlib/src/fs.ls`. Paths are owned `String` values, as everywhere
in the 2.0.0 library, and every function that can fail returns
`Result<T, io::Error>`. The `io::Result<T>` alias itself is not implemented in
2.0.0 and is planned for a later release; see [io](io.md).

## Implemented API

| Function | Signature | Meaning |
| --- | --- | --- |
| `read_to_string` | `(String) -> Result<String, io::Error>` | reads a UTF-8 file; invalid UTF-8 is an error |
| `write` | `(String, String) -> Result<(), io::Error>` | writes (truncates) a text file |
| `read` | `(String) -> Result<Vec<u8>, io::Error>` | reads a whole file as raw bytes |
| `append` | `(String, String) -> Result<(), io::Error>` | appends text, creating the file when missing |
| `copy` | `(String, String) -> Result<(), io::Error>` | copies a UTF-8 text file |
| `exists` | `(String) -> bool` | whether the path exists |
| `is_file` / `is_dir` | `(String) -> bool` | whether the path is a regular file / a directory |
| `metadata` | `(String) -> Result<Metadata, io::Error>` | size, modification time and kind |
| `read_dir` | `(String) -> Result<Vec<String>, io::Error>` | entry names, sorted, without `.` and `..` |
| `create_dir` | `(String) -> Result<(), io::Error>` | creates one directory |
| `create_dir_all` | `(String) -> Result<(), io::Error>` | creates a directory tree |
| `remove_file` | `(String) -> Result<(), io::Error>` | removes a file |
| `remove_dir` | `(String) -> Result<(), io::Error>` | removes an empty directory |
| `remove_dir_all` | `(String) -> Result<(), io::Error>` | removes a directory tree |
| `rename` | `(String, String) -> Result<(), io::Error>` | renames or moves |
| `canonicalize` | `(String) -> Result<String, io::Error>` | absolute, normalized path |
| `current_dir` | `() -> Result<String, io::Error>` | current working directory |
| `set_current_dir` | `(String) -> Result<(), io::Error>` | changes the working directory |
| `temp_dir` | `() -> String` | the system temporary directory (infallible) |

`Metadata` is a plain struct with `size`, `modified_ms` and `kind` fields and
five accessors:

| Method | Returns | Meaning |
| --- | --- | --- |
| `size` | `i64` | file size in bytes (`0` for directories) |
| `modified_ms` | `i64` | last modification time in milliseconds since the Unix epoch |
| `kind` | `i64` | `0` regular file, `1` directory, `2` anything else |
| `is_file` | `bool` | `kind == 0` |
| `is_dir` | `bool` | `kind == 1` |

`append` and `copy` are text-level helpers implemented in lsaca in 2.0.0: they
read the file and write it back, are not atomic and handle UTF-8 text only.
`read` is binary-safe because it goes through the `File` handle API.

## Error classification

The path-level functions report classified failures: a missing path is
`ErrorKind::NotFound`, a permission failure is `ErrorKind::PermissionDenied`
and an existing destination for `create_dir` is `ErrorKind::AlreadyExists`.
`read_to_string` and `write` are thin wrappers over the legacy text
intrinsics, so their failures carry `ErrorKind::Other` with the OS message.
The exact OS message is platform-specific and should not be parsed; use
`kind()` instead.

## Cross-platform behaviour

The functions are implemented in lsaca on top of the runtime file primitives, so
they behave the same on Windows, macOS and Linux within the documented error
kinds. Paths are text; a path that the platform cannot represent is a failure,
not a panic.

## Limits in 2.0

There is no recursive walker, no globbing, no file watcher and no symbolic-link
API in 2.0.0. `append` and `copy` are text-level and not atomic;
`metadata` reports only size, modification time and kind (no permissions or
owner), and directory entries are returned as names, not full paths.

## See also

* [io](io.md) and [error](error.md).
* [Standard library chapter](../language/15-standard-library.md#io-and-process) -
  `read_file` and `write_file` of 1.7.
