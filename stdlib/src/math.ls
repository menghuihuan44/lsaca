// Floating point helper functions.
//
// These are thin intrinsics: the compiler expands each call inline, exactly
// like the legacy builtin table did before the standard library took over the
// names.
//
// `abs`, `min` and `max` are deliberately still resolved by the compiler
// fallback table: they are generic over every numeric type, and a generic
// function currently hides a local variable of the same name in ordinary value
// position (probe: `let max: i64 = 3; println!("{}", max);` fails with E0423).
// They move here once that resolution-order bug is fixed.

pub extern fn sqrt(value: f64) -> f64;
pub extern fn floor(value: f64) -> f64;
pub extern fn ceil(value: f64) -> f64;
pub extern fn round(value: f64) -> f64;
pub extern fn sin(value: f64) -> f64;
pub extern fn cos(value: f64) -> f64;
pub extern fn tan(value: f64) -> f64;
pub extern fn ln(value: f64) -> f64;
pub extern fn log10(value: f64) -> f64;
pub extern fn exp(value: f64) -> f64;

pub extern fn pow(base: f64, exponent: f64) -> f64;
