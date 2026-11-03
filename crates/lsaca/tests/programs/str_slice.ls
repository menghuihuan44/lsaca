fn main() {
    let text = String::from("héllo wörld");
    println!("{}", &text[..6]);
    println!("{}", &text[7..]);
    println!("{}", &text[0..1]);
    println!("{}", &text[..]);

    let view: &str = &text;
    println!("{} {}", view.len(), view.char_count());
    println!("{} {}", view.starts_with("héllo"), view.ends_with("wörld"));
    println!("{}", view.to_uppercase());
    println!("{}", view.find("wörld").unwrap());
}
