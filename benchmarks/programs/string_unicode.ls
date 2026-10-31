fn mix(fold: i64, value: i64) -> i64 {
    (fold * 131 + value) % 1000000007
}

fn main() {
    let mut alphabet: Vec<char> = Vec::new();
    alphabet.push('a');
    alphabet.push('\u{E9}');
    alphabet.push('\u{F1}');
    alphabet.push('\u{FC}');
    alphabet.push('\u{F6}');
    alphabet.push('\u{DF}');
    alphabet.push('\u{FB01}');
    alphabet.push('\u{E0}');
    alphabet.push('\u{E7}');
    alphabet.push('\u{3B1}');
    alphabet.push('\u{3B2}');
    alphabet.push('\u{3B3}');
    alphabet.push('\u{3B4}');
    alphabet.push('\u{430}');
    alphabet.push('\u{431}');
    alphabet.push('\u{432}');
    alphabet.push('\u{4F60}');
    alphabet.push('\u{597D}');
    alphabet.push('\u{4E16}');
    alphabet.push('\u{754C}');
    alphabet.push('\u{1F600}');
    alphabet.push('k');
    alphabet.push('q');
    alphabet.push('w');
    let n = 120000;
    let k = alphabet.len();
    let mut base = String::from("");
    let mut i = 0;
    while i < n {
        let c = alphabet.get(i % k).unwrap();
        base.push(c);
        i += 1;
    }
    let mut fold = 0;
    let base_chars = base.chars();
    let mut index = 0;
    while index < base_chars.len() {
        fold = mix(fold, base_chars.get(index).unwrap() as i64);
        index += 1;
    }
    let mut probe = 0;
    while probe < 1000 {
        fold = mix(fold, base.char_at(probe * 7).unwrap() as i64);
        probe += 1;
    }
    let upper = base.to_uppercase();
    let upper_chars = upper.chars();
    index = 0;
    while index < upper_chars.len() {
        fold = mix(fold, upper_chars.get(index).unwrap() as i64);
        index += 1;
    }
    let mut mid = base.len() / 2;
    while !base.is_char_boundary(mid) {
        mid -= 1;
    }
    let mut end = mid + 5000;
    while !base.is_char_boundary(end) {
        end -= 1;
    }
    let mut built = base.substring(mid, end - mid);
    built.push('!');
    let mut appended = 0;
    while appended < 1000 {
        let c = upper_chars.get(appended).unwrap();
        built.push(c);
        appended += 1;
    }
    let popped = built.pop().unwrap();
    fold = mix(fold, popped as i64);
    built.push_str("done");
    let built_chars = built.chars();
    index = 0;
    while index < built_chars.len() {
        fold = mix(fold, built_chars.get(index).unwrap() as i64);
        index += 1;
    }
    println!("checksum={},{},{}", built.char_count(), built.len(), fold);
}
