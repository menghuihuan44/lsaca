// Real libm bindings. frexp takes an out-parameter through a raw pointer;
// the second call uses MaybeUninit for the same purpose.
use std::mem::MaybeUninit;

extern "C" {
    fn frexp(value: f64, exponent: *mut i32) -> f64;
    fn tgamma(value: f64) -> f64;
    fn erf(value: f64) -> f64;
}

fn main() {
    let mut exponent: i32 = 0;
    let fraction: f64 = unsafe { frexp(8.0, &mut exponent as *mut i32) };
    println!("frexp {:.1} {}", fraction, exponent);

    let mut slot: MaybeUninit<i32> = MaybeUninit::<i32>::uninit();
    let half: f64 = unsafe { frexp(0.5, slot.as_mut_ptr()) };
    let half_exponent: i32 = unsafe { slot.assume_init() };
    println!("frexp uninit {:.1} {}", half, half_exponent);

    let gamma: f64 = unsafe { tgamma(5.0) };
    println!("tgamma {:.4}", gamma);

    let error: f64 = unsafe { erf(1.0) };
    println!("erf {:.4}", error);
}
