// Integer-to-char casts are only allowed from u8; literals are rejected too.
fn main() {
    let value: char = 1000 as char;
    println!("{}", value);
}
