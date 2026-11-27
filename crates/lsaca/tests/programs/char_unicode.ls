// Unicode character classification and case conversion.
// Coverage: ASCII, Latin-1, Greek, Cyrillic, CJK, sharp s and dotted I.

fn main() {
    println!("{} {} {}", 'a'.is_alphabetic(), 'Z'.is_alphabetic(), '1'.is_alphabetic());
    println!("{} {} {}", '1'.is_numeric(), 'a'.is_numeric(), '\u{00B2}'.is_numeric());
    println!("{} {} {}", ' '.is_whitespace(), '\t'.is_whitespace(), '!'.is_whitespace());
    println!("{} {}", 'é'.is_alphabetic(), 'É'.is_alphabetic());
    println!("{} {}", 'ß'.is_alphabetic(), '\u{00B2}'.is_alphabetic());
    println!("{} {}", 'Ω'.is_alphabetic(), 'ω'.is_alphabetic());
    println!("{} {}", 'Ж'.is_alphabetic(), 'я'.is_alphabetic());
    println!("{} {}", '中'.is_alphabetic(), '。'.is_alphabetic());
    println!("{} {}", 'Ⅷ'.is_alphabetic(), 'Ⅷ'.is_numeric());
    println!("{} {}", '\u{0345}'.is_alphabetic(), '\u{05B0}'.is_alphabetic());
    println!("{} {} {}", '\u{00A0}'.is_whitespace(), '\u{2003}'.is_whitespace(), '\u{200B}'.is_whitespace());
    println!("{} {} {}", '\u{3000}'.is_whitespace(), '\u{2028}'.is_whitespace(), '\u{1680}'.is_whitespace());
    println!("{} {} {} {}", 'a'.is_alphanumeric(), '5'.is_alphanumeric(), 'é'.is_alphanumeric(), ' '.is_alphanumeric());

    println!("{} {}", 'a'.to_uppercase(), 'Z'.to_lowercase());
    println!("{} {} {}", 'é'.to_uppercase(), 'É'.to_lowercase(), 'ÿ'.to_uppercase());
    println!("{} {} {}", 'Ω'.to_lowercase(), 'ω'.to_uppercase(), 'ς'.to_uppercase());
    println!("{} {} {}", 'Ж'.to_lowercase(), 'я'.to_uppercase(), 'ё'.to_uppercase());
    println!("{} {}", '山'.to_uppercase(), '中'.to_lowercase());

    // One-to-many mappings keep the first (closest) scalar in char results.
    println!("{} {}", 'ß'.to_uppercase(), 'ß'.to_uppercase() as u32);
    println!("{} {}", 'İ'.to_lowercase(), 'İ'.to_lowercase() as u32);
    println!("{} {}", 'ı'.to_uppercase(), 'ı'.to_uppercase() as u32);
    println!("{} {}", 'ﬁ'.to_uppercase(), 'ﬁ'.to_uppercase() as u32);
    println!("{} {}", 'ẞ'.to_lowercase(), 'ẞ'.to_lowercase() as u32);

    // String case mapping still expands the full mapping.
    println!("{}", String::from("ß").to_uppercase());
}
