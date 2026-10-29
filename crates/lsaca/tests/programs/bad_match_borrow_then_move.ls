fn main() {
    let maybe = Some(String::from("hi"));
    match &maybe {
        Some(value) => {
            let moved = maybe;
            let n = value.len();
        }
        None => {}
    }
}
