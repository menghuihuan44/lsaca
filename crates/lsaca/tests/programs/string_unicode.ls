fn main() {
    let text = String::from("héllo 你好");
    println!("{}", text.len());
    println!("{}", text.char_count());
    println!("{} {}", text.is_char_boundary(1), text.is_char_boundary(2));
    println!("{:?}", text.char_at(0));
    println!("{:?}", text.char_at(1));
    println!("{:?}", text.char_at(5));
    println!("{}", text.substring(7, 6));
    let mut scratch = text.substring(0, 5);
    scratch.push('!');
    let popped = scratch.pop().unwrap();
    println!("{} {}", scratch, popped);
    println!("{}", "straße".to_uppercase());
    println!("{}", "HÉLLO".to_lowercase());
    println!("[{}]", "\u{A0}x\u{A0}".trim());
    println!("{} {}", "".char_count(), "".is_empty());
    let chars = "abc".chars();
    println!("{}", chars.len());
    let mut total = 0;
    for index in 0..chars.len() {
        let literal = '0';
        let character = chars.get(index).unwrap();
        if character >= literal {
            total = total + 1;
        }
    }
    println!("{}", total);
    let combined = String::from_char('x');
    println!("{} {:?}", combined, combined);
}
