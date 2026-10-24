// Format specifiers are validated against the argument type at compile time.
fn main() {
    println!("{:q}", 1);
    println!("{:.2}", 1);
    println!("{:05}", String::from("x"));
}
