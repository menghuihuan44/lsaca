fn main() {
    let text = String::from("  hello, world  ");
    println!("{:?}", text.trim());
    println!("{:?}", text.trim_start());
    println!("{:?}", text.trim_end());
    println!("{:?}", text.split_whitespace());
    println!("{:?}", "a\nb\nc".lines());
    println!("{}", "abc".chars().len());
    println!("{}", text.replace("world", "lsaca"));
    println!("{}", 'A'.to_ascii_lowercase());
    println!("{}", '7'.to_digit(10).unwrap());
    match "42".parse_i64() {
        Ok(value) => println!("parsed {}", value),
        Err(error) => println!("error {}", error.message()),
    }
    println!("{}", "x".parse_f64().is_err());
}
