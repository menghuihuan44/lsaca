# math: math functions

**Status in 2.0.0.** The math free functions are migrated from the builtin prelude
into the `std` crate (`std::math`) without surface changes: the unqualified names keep
working, and the qualified path `std::math::sqrt` names the same function. The eleven
float functions (`sqrt`, `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `ln`,
`log10`, `exp`, `pow`) are declared in `stdlib/src/math.ls`; `abs`, `min` and
`max` are still provided by the compiler's builtin table, so they work unqualified
but are not items of `std::math` in 2.0.0.

The functions are free functions, not methods, and they are always in scope through
the prelude. They map directly to the corresponding C math library functions: no
operator overloading, no implicit conversions and no complex numbers.

| Function | Signature | Meaning |
| --- | --- | --- |
| `sqrt(x)` | `f64 -> f64` | square root |
| `abs(x)` | integers or `f64` | absolute value |
| `min(a, b)` / `max(a, b)` | integers or `f64` | smaller/larger value |
| `pow(a, b)` | `(f64, f64) -> f64` | `a` raised to `b` (float only) |
| `floor(x)`, `ceil(x)`, `round(x)` | `f64 -> f64` | rounding |
| `sin(x)`, `cos(x)`, `tan(x)` | `f64 -> f64` | trigonometry |
| `ln(x)`, `log10(x)`, `exp(x)` | `f64 -> f64` | natural log, base-10 log, exponential |

Integer arguments are converted explicitly with `as` before a float-only function;
`pow(2, 10)` is a type error (`E0308`), and `2.0.pow(10.0)` is not a method. There is
no integer exponentiation function in 2.0.

## Planned for a later release

* `abs`, `min` and `max` are planned for a later release: they will move from the
  compiler's builtin table into `std::math`; their signatures do not change.
* No new math functions are added in 2.0.0, and constants such as `PI` and `E` are not
  provided; write the literal.

## See also

* [Standard library chapter](../language/15-standard-library.md#math) - the 1.7
  reference of the free functions.
