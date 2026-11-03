# fmt: formatting

**Status in 2.0.0.** The format machinery is implemented by the compiler. `std::fmt` is
the module that documents the supported surface; it does not expose
user-implementable formatting traits in 2.0.

## Macros

| Macro | Output |
| --- | --- |
| `print!(fmt, args…)` / `println!(fmt, args…)` | stdout |
| `eprint!(fmt, args…)` / `eprintln!(fmt, args…)` | stderr |
| `format!(fmt, args…)` | returns a `String` |
| `panic!(fmt, args…)` | aborts with a panic message |

The braces in the format string are a mini-language: `{}` takes the next argument, a
literal `{` or `}` is written `{{` or `}}`, and the format specifier grammar is
documented in the [Standard library chapter](../language/15-standard-library.md#format-specifier-grammar).

## Format specifiers

The grammar is `[[fill]align][sign][#][0][width][.precision][type]`:

* fill and alignment: `<`, `>`, `^` with an optional fill character;
* sign: `+` and the space flag;
* `#` alternate form;
* `0` zero padding (needs a width);
* width and precision as counts of **characters**, not bytes;
* types: `?` for Debug, `x` / `X` / `o` / `b` for integers, `e` / `E` for floats.

Width counts characters, not bytes, so a multi-byte character occupies one column.
A formatting error (for example a type that does not support the requested
specifier) is a compile-time error, not a run-time panic.

## Debug and Display

There are no user-implementable `Display`/`Debug` traits in 2.0.0. Every value prints
through the formatting rules the compiler knows: structs and enums print
field/variant names, `String` prints its text, `char` prints with quotes and escapes
in Debug, and `?` uses the escaped form. Trait objects print through their vtable
(see [Trait objects](../language/25-trait-objects.md)).

## Planned for a later release

* Formatting helpers (`join`-style utilities) are not planned; the macros and the
  compiler rules are the whole surface.
* User-defined `Display`/`Debug` implementations are deferred; `impl` on a trait
  that is not `Drop` remains an error.

## See also

* [Standard library chapter](../language/15-standard-library.md#formatting-and-printing)
  - full grammar, width/precision and debug-escaping reference.
