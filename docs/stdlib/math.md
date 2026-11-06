# math: math functions and constants

**Status in 2.1.0.** The math free functions live in `stdlib/src/math.ls` and are
re-exported from the crate root, so the unqualified names keep working and the
qualified path `std::math::sqrt` names the same function. The eleven float
functions (`sqrt`, `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `ln`,
`log10`, `exp`, `pow`) are declared there, together with sixteen `f64`
constants. `abs`, `min` and `max` are still provided by the compiler's builtin
table, so they work unqualified but are not items of `std::math`.

The functions are free functions, not methods. They map directly to the
corresponding C math library functions: no operator overloading, no implicit
conversions and no complex numbers.

| Function | Signature | Meaning |
| --- | --- | --- |
| `sqrt(x)` | `f64 -> f64` | square root |
| `abs(x)` | integers or `f64` | absolute value (builtin) |
| `min(a, b)` / `max(a, b)` | integers or `f64` | smaller/larger value (builtin) |
| `pow(a, b)` | `(f64, f64) -> f64` | `a` raised to `b` (float only) |
| `floor(x)`, `ceil(x)`, `round(x)` | `f64 -> f64` | rounding |
| `sin(x)`, `cos(x)`, `tan(x)` | `f64 -> f64` | trigonometry |
| `ln(x)`, `log10(x)`, `exp(x)` | `f64 -> f64` | natural log, base-10 log, exponential |

Integer arguments are converted explicitly with `as` before a float-only function;
`pow(2, 10)` is a type error (`E0308`), and `2.0.pow(10.0)` is not a method. There is
no integer exponentiation function.

## Constants

The constants are `f64` values with the exact Rust values. They are visible both
unqualified through the prelude and as `std::math::…` paths.

| Constant | Meaning |
| --- | --- |
| `PI` | Archimedes' constant |
| `E` | Euler's number |
| `TAU` | the circle constant, `2 * PI` |
| `SQRT_2`, `FRAC_1_SQRT_2` | the square root of two and its reciprocal |
| `FRAC_1_PI`, `FRAC_2_PI`, `FRAC_2_SQRT_PI` | reciprocals and multiples of `PI` |
| `LN_2`, `LN_10` | natural logarithms of two and ten |
| `LOG2_E`, `LOG10_E` | logarithms of `E` in base two and base ten |
| `PHI` | the golden ratio |
| `FRAC_PI_2`, `FRAC_PI_3`, `FRAC_PI_4` | `PI` divided by two, three and four |

Constants are values, not functions: write `PI`, not `PI()`, and they must be
printed through a float specifier where a decimal form is wanted.

## Planned for a later release

* `abs`, `min` and `max` are planned to move from the compiler's builtin table
  into `std::math`; their signatures do not change.
* No other math functions are planned.

## See also

* [Standard library chapter](../language/15-standard-library.md#math) - the
  language-level reference of the free functions.
