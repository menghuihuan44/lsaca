fn describe(letter: char) -> String {
    match letter {
        'a' => String::from("letter a"),
        '0' | '1' | '2' => String::from("small digit"),
        ' ' => String::from("space"),
        _ => String::from("other"),
    }
}

fn main() {
    let letter = 'a';
    let newline = '\n';
    let han = '\u{4F60}';
    println!("{} {} {}", letter, newline == '\n', han);
    println!("{:?}", newline);
    println!("{}", letter as i64);
    println!("{}", letter < 'b');
    println!("{}", describe('a'));
    println!("{}", describe('2'));
    println!("{}", describe(' '));
    println!("{}", describe('z'));
    let mut chars: Vec<char> = Vec::new();
    chars.push('x');
    chars.push(han);
    println!("{} {}", chars.len(), chars.get(1).unwrap());
    let some: Option<char> = Some('z');
    match some {
        Some(value) => println!("some {}", value),
        None => println!("none"),
    }
    let text = String::from_char(han);
    println!("{} {}", text, text.char_count());
    println!("{}", text.char_at(0).unwrap());
}
