// ASCII character classification through the stdlib `impl char` methods.

fn main() {
    println!("{} {} {} {}",
        'a'.is_ascii_alphabetic(),
        '5'.is_ascii_digit(),
        '_'.is_ascii_word(),
        '!'.is_ascii_word());
    println!("{} {}", 'A'.is_ascii_uppercase(), 'A'.is_ascii_lowercase());
    println!("{} {} {}", 'f'.is_ascii_hexdigit(), 'g'.is_ascii_hexdigit(), ' '.is_ascii_whitespace());

    let mut letters: i64 = 0;
    let mut digits: i64 = 0;
    let mut spaces: i64 = 0;
    let mut chars: Vec<char> = Vec::new();
    chars.push('x');
    chars.push('1');
    chars.push(' ');
    chars.push('Z');
    chars.push('!');
    for c in chars {
        if c.is_ascii_alphabetic() { letters = letters + 1; }
        if c.is_ascii_digit() { digits = digits + 1; }
        if c.is_ascii_whitespace() { spaces = spaces + 1; }
    }
    println!("{} {} {}", letters, digits, spaces);

    let text: String = String::from("Hello, Wörld 42");
    let mut word_chars: i64 = 0;
    let mut text_chars: Vec<char> = text.chars();
    for c in text_chars {
        if c.is_ascii_word() { word_chars = word_chars + 1; }
    }
    println!("{}", word_chars);
}
