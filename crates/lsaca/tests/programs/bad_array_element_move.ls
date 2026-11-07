// Array elements are places; a non-Copy element cannot be moved out (E0507).
fn main() {
    let words: [String; 2] = [String::from("a"), String::from("b")];
    let first: String = words[0];
    println!("{}", first);
}
