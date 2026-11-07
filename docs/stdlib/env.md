# env: environment variables

**Status in 2.2.0.** Implemented. The module is implemented in
`stdlib/src/env.ls`. The environment is read-only: there is no `set_var`, no
`remove_var` and no way to change the environment of another process.

| Function | Signature | Meaning |
| --- | --- | --- |
| `var` | `(name: &str) -> Result<String, VarError>` | the value of the variable |
| `vars` | `() -> Vec<String>` | every environment entry as a `"NAME=value"` string |

`var` takes a borrowed name. Since 2.1.0 a string literal is a static `&str`
slice, so `env::var("PATH")` allocates nothing; a `String` variable is passed as
`&name`. Reading is fallible: a variable that is not set is
`Err(VarError::NotPresent)`, never a panic.

`VarError` has two variants:

| Variant | Meaning |
| --- | --- |
| `NotPresent` | the variable is not set |
| `NotUnicode(String)` | the value cannot be represented as text (reserved; the current runtime always reports text, so it is not produced yet) |

`vars` returns the entries as `"NAME=value"` text; the order is the order the
operating system reports.

## Cross-platform notes

Environment data is text: names and values are read as UTF-8 strings. On POSIX
systems the operating system stores environment entries as bytes, so an entry
that is not valid UTF-8 cannot be represented; on Windows the environment is
Unicode and is read directly. Variable names are case-sensitive on POSIX and
case-insensitive on Windows, so portable programs should use one spelling
consistently and should not rely on two names differing only by case.

## Limits in 2.2.0

There is no environment-file loading, no process spawning and no
platform-specific extension.

## See also

* [process](process.md) and [time](time.md).
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
