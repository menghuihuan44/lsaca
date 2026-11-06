// No implicit integer conversion: an i32 value cannot be added to an i64 value (v2.1).
fn main() {
    let small: i32 = 1;
    let big: i64 = 2;
    let total = small + big;
    println!("{}", total);
}
