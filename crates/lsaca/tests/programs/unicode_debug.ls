fn main() {
    println!("{:?}", 'x');
    println!("{:?}", '\n');
    println!("{:?}", '\'');
    println!("{:?}", '\\');
    println!("{:?}", '\u{7}');
    println!("{:?}", '你');
    println!("{:?}", "a\u{0}b");
    println!("{:?}", "tab\there");
    println!("{:?}", "");
    println!("[{}]", '\u{1F600}');
}
