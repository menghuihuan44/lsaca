// Generic parse accepts only i64 and f64 targets in 2.1.0.
fn main() {
    let parsed = "42".parse::<u8>();
    println!("{:?}", parsed);
}
