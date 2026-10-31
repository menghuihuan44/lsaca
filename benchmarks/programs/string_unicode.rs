// string_unicode: exercise UTF-8 string handling over a deterministic
// mixed-ASCII/Unicode corpus: build 120,000 scalars one at a time, fold the
// scalars from iteration and random access, apply full-Unicode to_uppercase,
// slice at character boundaries, extend, pop and fold again.
// The checksum is <char_count>,<byte_len>,<folded scalar checksum>.
const MOD: i64 = 1000000007;

fn mix(fold: i64, value: i64) -> i64 {
    (fold * 131 + value) % MOD
}

fn main() {
    let alphabet: Vec<char> = "a\u{e9}\u{f1}\u{fc}\u{f6}\u{df}\u{fb01}\u{e0}\u{e7}\u{3b1}\u{3b2}\u{3b3}\u{3b4}\u{430}\u{431}\u{432}\u{4f60}\u{597d}\u{4e16}\u{754c}\u{1f600}kqw".chars().collect();
    let n = 120000usize;
    let k = alphabet.len();
    let base: Vec<char> = (0..n).map(|i| alphabet[i % k]).collect();
    let mut fold: i64 = 0;
    for &cp in &base {
        fold = mix(fold, cp as i64);
    }
    for probe in 0..1000 {
        fold = mix(fold, base[probe * 7] as i64);
    }
    let data: String = base.iter().collect();
    let upper: String = data.to_uppercase();
    for cp in upper.chars() {
        fold = mix(fold, cp as i64);
    }
    let mut mid = data.len() / 2;
    while !data.is_char_boundary(mid) {
        mid -= 1;
    }
    let mut end = mid + 5000;
    while !data.is_char_boundary(end) {
        end -= 1;
    }
    let mut built: String = data[mid..end].to_string();
    built.push('!');
    for cp in upper.chars().take(1000) {
        built.push(cp);
    }
    let popped = built.pop().unwrap();
    fold = mix(fold, popped as i64);
    built.push_str("done");
    for cp in built.chars() {
        fold = mix(fold, cp as i64);
    }
    println!("checksum={},{},{}", built.chars().count(), built.len(), fold);
}
