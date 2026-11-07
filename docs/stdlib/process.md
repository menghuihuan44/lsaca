# process: arguments and exit

**Status in 2.2.0.** Implemented. Both functions are defined in
`stdlib/src/process.ls` and re-exported from the crate root, so `args()` and
`exit(code)` keep working unqualified and `std::process::args` /
`std::process::exit` name the same items.

| Function | Signature | Meaning |
| --- | --- | --- |
| `args` | `() -> Vec<String>` | command-line arguments after the program name |
| `exit` | `(code: i32)` | terminates the process with the given exit code |

`process::args` returns the same argument vector as the 1.7 free function
`args` (see [the standard library
chapter](../language/15-standard-library.md#io-and-process)): the program name is
not included, so the first user argument is index `0`. Arguments are text; an
argument that is not valid UTF-8 makes the call panic with
`arguments contain invalid UTF-8`, the same rule as in 1.7.

`process::exit` terminates the process immediately. Nothing after the call runs,
pending `Drop` hooks are not executed and the reference-count leak report is
skipped - exactly like the 1.7 `exit` free function (see [Ownership and
moves](../language/11-ownership-and-moves.md#the-drop-trait)). It is declared as
returning unit; at run time it never returns to the caller. The exit code is an
`i32` (the Rust type; the pre-2.1 `i64` parameter is gone) and becomes the
process exit status; `0` means success by convention, and positive small codes
are the portable choice because POSIX systems report only the low 8 bits.

## Limits in 2.2.0

There is no `spawn`, no `wait`, no pipe or signal API and no way to clear the
environment of a child process.

## Cross-platform notes

On Windows the command line is Unicode and is surfaced as text; on POSIX the
argument bytes must be valid UTF-8 for the `args` vector. The exit code is passed
to the operating system unchanged; how a shell reports a large or negative code is
platform-specific.

## See also

* [env](env.md) and [time](time.md).
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
