// Characters, UTF-8 strings and Unicode-aware methods (lsaca 1.6).
//
// Run with:
//     lsaca run examples/unicode.ls

fn describe(text: &String) {
    println!("text = {:?}", text);
    println!("bytes = {}, characters = {}", text.len(), text.char_count());
    match text.char_at(0) {
        Some(first) => println!("first = {:?}", first),
        None => println!("empty"),
    }
}

fn main() {
    let text = String::from("Héllo, 世界");
    describe(&text);

    println!("upper = {}", text.to_uppercase());
    println!("lower = {}", String::from("ÉCOLE").to_lowercase());
    println!("sharp s = {}", String::from("straße").to_uppercase());

    let padded = String::from("  \u{3000} spaced \u{3000}  ");
    let trimmed = padded.trim();
    println!("trimmed = {:?}", trimmed);

    let pieces = text.chars();
    println!("chars = {:?}", pieces);
    println!("char count = {}", pieces.len());

    let start = text.find("世").unwrap_or(0);
    let end = start + 3;
    if text.is_char_boundary(start) && text.is_char_boundary(end) {
        println!("substring = {}", text.substring(start, 3));
    }

    let mut greeting = String::from("hi");
    greeting.push('!');
    greeting.push('😀');
    println!("greeting = {:?} ({} characters)", greeting, greeting.char_count());
    let last = greeting.pop();
    println!("popped = {:?}; now {:?}", last, greeting);

    let first = String::from_char('ñ');
    println!("from_char = {}", first);

    println!("{} {}", 'A' as i64, '你' as u32);
    println!("{:?} {:?}", '\n', '😀');
}
