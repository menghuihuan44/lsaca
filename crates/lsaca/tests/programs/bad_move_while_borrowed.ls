// Moving a value while a shared borrow of it is live is rejected (E0505).
fn main() {
    let text = String::from("lsaca");
    let borrowed = &text;
    let moved = text;
    println!("{} {}", borrowed, moved);
}
