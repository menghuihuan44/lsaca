// Default integer type is i32; explicit suffixes select other types (v2.1).
fn takes_i32(value: i32) -> i64 {
    value as i64
}

fn main() {
    let a = 1 + 2;
    println!("{}", takes_i32(a));

    let b: u8 = 255u8;
    let c: i8 = -128i8;
    let d = 2_000_000_000i64;
    let e = 1.5f32;
    let f = 3.25f64;
    println!("{} {} {} {}", b, c, d, e as f64);
    println!("{}", f);

    let g = 0xffu32;
    let h = 0b1010u16;
    let i = 0o77i32;
    println!("{} {} {}", g, h, i);

    let typed: i64 = 3000000000;
    let index = 1;
    let values = vec![10i64, 20i64, 30i64];
    println!("{} {}", typed, values[index]);
}