// usize values do not implicitly convert to i64 (v2.1).
fn main() {
    let index: usize = 2;
    let value: i64 = index;
    println!("{}", value);
}
