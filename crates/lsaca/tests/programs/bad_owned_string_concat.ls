// Two owned String values cannot be added; only String + &str concatenates (v2.1).
fn main() {
    let left: String = String::from("a");
    let right: String = String::from("b");
    let joined = left + right;
    println!("{}", joined);
}
