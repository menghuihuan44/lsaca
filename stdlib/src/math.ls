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
// Mathematical constants (v2.1, exact Rust `f64` values).
//
// Rust exposes these through `std::f64::consts`; lsaca has no associated
// constants on primitive types, so they live here as module constants and are
// re-exported by the crate root.

/// Archimedes' constant, pi.
pub const PI: f64 = 3.141592653589793;
/// Euler's number.
pub const E: f64 = 2.718281828459045;
/// The full circle constant, 2 * pi.
pub const TAU: f64 = 6.283185307179586;
/// The square root of two.
pub const SQRT_2: f64 = 1.4142135623730951;
/// The reciprocal of the square root of two.
pub const FRAC_1_SQRT_2: f64 = 0.7071067811865476;
/// The reciprocal of pi.
pub const FRAC_1_PI: f64 = 0.3183098861837907;
/// Two times the reciprocal of pi.
pub const FRAC_2_PI: f64 = 0.6366197723675814;
/// Two divided by the square root of pi.
pub const FRAC_2_SQRT_PI: f64 = 1.1283791670955126;
/// The natural logarithm of two.
pub const LN_2: f64 = 0.6931471805599453;
/// The natural logarithm of ten.
pub const LN_10: f64 = 2.302585092994046;
/// The base two logarithm of Euler's number.
pub const LOG2_E: f64 = 1.4426950408889634;
/// The base ten logarithm of Euler's number.
pub const LOG10_E: f64 = 0.4342944819032518;
/// The golden ratio.
pub const PHI: f64 = 1.618033988749895;
/// Half of pi.
pub const FRAC_PI_2: f64 = 1.5707963267948966;
/// One third of pi.
pub const FRAC_PI_3: f64 = 1.0471975511965976;
/// One quarter of pi.
pub const FRAC_PI_4: f64 = 0.7853981633974483;
