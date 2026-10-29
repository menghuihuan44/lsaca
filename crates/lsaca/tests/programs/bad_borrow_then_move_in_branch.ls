fn main() {
    let text = String::from("lsaca");
    let borrowed = &text;
    if borrowed.len() > 3 {
        let moved = text;
        let after = borrowed.len();
        println!("{} {}", moved, after);
    }
}
