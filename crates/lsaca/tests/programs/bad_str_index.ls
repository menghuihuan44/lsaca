// A str value cannot be indexed by a byte position (v2.1).
fn main() {
    let text: &str = "hello";
    let first = text[0];
    println!("{}", first);
}
