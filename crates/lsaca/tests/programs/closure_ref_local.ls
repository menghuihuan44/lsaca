fn is_long(value: &String) -> bool {
    value.len() > 3
}

fn find_long(text: String) -> Option<String> {
    if is_long(&text) {
        return Some(text);
    }
    None
}

fn main() {
    let text = String::from("lsaca");
    // The explicit return type keeps this a plain bool predicate: the
    // closure returns a comparison, not the compared value.
    let keep = |value: &String| -> bool { value.len() > 3 };
    if keep(&text) {
        let moved = text;
        println!("{}", moved);
    }

    match find_long(String::from("alpha")) {
        Some(value) => println!("{}", value),
        None => println!("none"),
    }
}
