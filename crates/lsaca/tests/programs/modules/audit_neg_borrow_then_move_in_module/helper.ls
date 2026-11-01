pub fn moved_while_borrowed() -> i64 {
    let text = String::from("x");
    let reference = &text;
    let moved = text;
    reference.len() + moved.len()
}
