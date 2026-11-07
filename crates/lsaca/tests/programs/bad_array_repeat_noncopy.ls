// Repeating a non-Copy operand is rejected when the length is greater than one (E0507).
fn main() {
    let word = String::from("x");
    let pair: [String; 2] = [word; 2];
    println!("{:?}", pair);
}
