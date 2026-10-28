fn append(text: &mut String) {
    text.push_str("!");
}

fn main() {
    let mut text = String::from("hi");
    append(&mut text);
    println!("{}", text);
}
