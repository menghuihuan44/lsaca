pub fn moved_while_borrowed() -> usize {
    let text = String::from("x");
    let reference = &text;
    let moved = text;
    reference.len() + moved.len()
}
