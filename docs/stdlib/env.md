# env: environment variables

**Status in 2.0.0.** Implemented. The module is implemented in
`stdlib/src/env.ls`. The environment is read-only: there is no `set_var`, no
`remove_var` and no way to change the environment of another process.

| Function | Signature | Meaning |
| --- | --- | --- |
| `var` | `(name: String) -> Option<String>` | value of the variable, or `None` when it is not set |
| `vars` | `() -> Vec<String>` | every environment entry as a `"NAME=value"` string |

`var` takes an owned `String`, like every other text parameter in the 2.0
library: string literals are `String` values, so `env::var("PATH")` works
directly, and a caller holding a `String` passes it by value. A missing variable
is `None`, never an error or a panic.

`vars` returns the entries as `"NAME=value"` text; the order is the order the
operating system reports.

## Cross-platform notes

Environment data is text: names and values are read as UTF-8 strings. On POSIX
systems the operating system stores environment entries as bytes, so an entry
that is not valid UTF-8 cannot be represented; on Windows the environment is
Unicode and is read directly. Variable names are case-sensitive on POSIX and
case-insensitive on Windows, so portable programs should use one spelling
consistently and should not rely on two names differing only by case.

## Limits in 2.0

There is no environment-file loading, no process spawning and no
platform-specific extension in 2.0.

## See also

* [process](process.md) and [time](time.md).
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
