// An array of owned values moves on assignment; the source is dead after it (E0382).
fn main() {
    let words: [String; 2] = [String::from("a"), String::from("b")];
    let moved = words;
    println!("{:?}", words);
}
