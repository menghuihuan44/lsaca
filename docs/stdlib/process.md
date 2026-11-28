# process: arguments, exit and child commands

**Status in 2.5.0.** Implemented. `args`, `exit` and the command types are
defined in `stdlib/src/process.ls`. `args()` and `exit(code)` are re-exported
from the crate root, so they keep working unqualified; the command types are
named through the module, as in `std::process::Command`.

| Function or type | Signature or purpose |
| --- | --- |
| `args` | `() -> Vec<String>`: command-line arguments after the program name |
| `exit` | `(code: i32)`: terminates the process with the given exit code |
| `Command` | a builder for running one child process |
| `ExitStatus` | the exit status of a finished child |
| `Output` | the captured status, standard output and standard error of a child |

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
`i32` and becomes the process exit status; `0` means success by convention, and
positive small codes are the portable choice because POSIX systems report only
the low 8 bits.

## Running a child process

Version 2.4.0 adds a single-command builder. It runs without a shell: the program
and every argument are passed to the operating system as one exact argument
vector, with no quoting rules, wildcard expansion or variable substitution:

```rust
let mut command = std::process::Command::new(String::from("program"));
command.arg(String::from("first argument"));
command.args(vec![String::from("second"), String::from("third")]);
command.current_dir(String::from("work"));
let status = command.status();
```

| Method | Signature | Meaning |
| --- | --- | --- |
| `Command::new` | `(program: String) -> Command` | creates a builder for `program` |
| `arg` | `(&mut self, argument: String)` | appends one argument |
| `args` | `(&mut self, arguments: Vec<String>)` | appends every argument in the vector, in order |
| `current_dir` | `(&mut self, directory: String)` | sets the working directory; the empty string inherits the parent's directory |
| `status` | `(&self) -> Result<ExitStatus, Error>` | runs the command with inherited standard streams and waits for it |
| `output` | `(&self) -> Result<Output, Error>` | runs the command, captures stdout and stderr, and waits for it |

The builder methods return nothing; call them as separate statements. They
mutate the command in place. A command can be run more than once; each call
starts a fresh child with the same program, arguments and directory.

`status` inherits the parent's standard input, standard output and standard
error, so the child writes directly to the terminal. `output` starts the child
with pipes for stdout and stderr and returns their complete contents in an
`Output` value. Both calls wait for the child to finish. A non-zero child exit
status is not an `Err`: both calls return `Ok` with the `ExitStatus`, and only a
child that could not be started (an empty program, a missing executable, an
invalid working directory, and so on) returns `Err`.

The error type is [`io::Error`](io.md), the same type used by the other
standard-library modules.

| Type | Method | Signature | Meaning |
| --- | --- | --- | --- |
| `ExitStatus` | `code` | `(&self) -> i32` | the child's exit code |
| `ExitStatus` | `success` | `(&self) -> bool` | true when `code() == 0` |
| `Output` | `status` | `(&self) -> ExitStatus` | the child's status |
| `Output` | `stdout` | `(&self) -> String` | captured standard output |
| `Output` | `stderr` | `(&self) -> String` | captured standard error |

`Output` getters clone their strings; the `Output` value owns the captured
text and may be dropped after reading it. Captured bytes that are not valid
UTF-8 are replaced with `?` by the runtime, so `output` always returns text.
There is no raw-byte accessor.

On POSIX a child killed by a signal reports `128 + signal` as its code; on
Windows `code` is the child's 32-bit exit code. A Windows command line is
Unicode, while POSIX passes argument bytes through as UTF-8.

## Limits in 2.5.0

There is no `spawn`, no separate `wait`, no way to write standard input, no pipe
or signal API, no environment clearing, and no asynchronous command API.
`status` and `output` always wait for the child. The language still has no
shell builtins or shell syntax in a command string: construct the argument
vector explicitly.

## Cross-platform notes

The program name is resolved by the operating system, following its normal
executable search rules; an explicit path works as usual. Argument strings must
be valid UTF-8 because the lsaca `String` type is UTF-8. The working directory
must exist when the command runs. Exit codes are passed through unchanged;
how a POSIX shell reports a large or negative code is platform-specific.

## See also

* [env](env.md) and [time](time.md).
* [io](io.md) - the `Error` type returned by `status` and `output`.
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
