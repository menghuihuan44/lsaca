// div is a real libc function returning a small C struct by value. DivT is
// repr(C) so its layout matches div_t on every platform.
#[repr(C)]
struct DivT {
    quot: i32,
    rem: i32,
}

extern "C" {
    fn div(numerator: i32, denominator: i32) -> DivT;
}

fn main() {
    let positive: DivT = unsafe { div(17, 5) };
    println!("quot {} rem {}", positive.quot, positive.rem);

    let negative: DivT = unsafe { div(-17, 5) };
    println!("negative quot {} rem {}", negative.quot, negative.rem);
}
