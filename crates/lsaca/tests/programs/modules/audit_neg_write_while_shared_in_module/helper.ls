pub fn write_while_shared() -> usize {
    let mut text = String::from("a");
    let reference = &text;
    text = String::from("b");
    reference.len()
}
