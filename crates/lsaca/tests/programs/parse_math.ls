fn main() {
    let n = "12".parse_i64().unwrap() + 30;
    println!("{}", n);
    println!("{}", sqrt(16.0));
    println!("{}", abs(-5));
    println!("{}", min(3, 9));
    println!("{}", max(3.9, 2.1));
}
