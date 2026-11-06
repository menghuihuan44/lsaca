// Two borrowed text views cannot be added (v2.1).
fn main() {
    let left: &str = "a";
    let right: &str = "b";
    let joined = left + right;
    println!("{}", joined);
}
