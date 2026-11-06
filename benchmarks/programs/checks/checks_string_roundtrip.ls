// Property check: building, splitting, slicing, appending, popping, uppercasing
// and parsing strings keeps byte length, character count and numeric values
// consistent (the text stays ASCII, so len() and char_count() must agree).
fn main() {
    let tokens: i64 = 500;
    let mut text = String::new();
    let mut index: i64 = 0;
    while index < tokens {
        let piece = format!("v{},", index);
        text.push_str(&piece);
        index += 1;
    }
    assert_eq!(text.char_count(), text.len());
    let parts = text.split(",");
    let mut seen: i64 = 0;
    let mut sum: i64 = 0;
    let mut longest: usize = 0;
    for part in parts {
        if part.len() > 0 {
            let value = part.substring(1, part.len() - 1).parse_i64().unwrap();
            assert_eq!(value, seen);
            sum += value;
            if part.len() > longest {
                longest = part.len();
            }
            seen += 1;
        }
    }
    assert_eq!(seen, tokens);
    assert_eq!(sum, tokens * (tokens - 1) / 2);
    assert_eq!(longest, 4);
    let upper = text.to_uppercase();
    assert_eq!(upper.len(), text.len());
    assert_eq!(upper.char_count(), text.char_count());
    let prefix = text.substring(0, 2);
    assert_eq!(prefix.char_count(), 2);
    assert_eq!(prefix.char_at(0).unwrap(), 'v');
    let mut built = String::from("done");
    built.push('!');
    assert_eq!(built.pop().unwrap(), '!');
    assert_eq!(built.len(), 4);
    let number = format!("{}", 123456789);
    assert_eq!(number.parse_i64().unwrap(), 123456789);
    println!("ok strings");
}
