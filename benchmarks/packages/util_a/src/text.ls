// text: string helpers for the util_a library package.

pub const MODULUS: i64 = 1000000007;

pub fn word_lengths(text: &String) -> Vec<i64> {
    let mut lengths: Vec<i64> = Vec::new();
    for word in text.split(" ") {
        if word.len() > 0 {
            lengths.push(word.len() as i64);
        }
    }
    return lengths;
}

pub fn list_words(text: &String) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in text.split(" ") {
        if word.len() > 0 {
            words.push(format!("{}", word.trim()));
        }
    }
    return words;
}

pub fn word_total(words: Vec<String>) -> i64 {
    let mut total = 0;
    for word in words {
        total = total + word.len() as i64;
    }
    return total;
}

pub fn repeat(text: &String, times: i64) -> String {
    return text.repeat(times as usize);
}

pub fn shout(text: &String) -> String {
    return text.to_uppercase();
}

pub fn whisper(text: &String) -> String {
    return text.to_lowercase();
}

pub fn csv_total(text: &String) -> i64 {
    let mut total = 0;
    for token in text.split(",") {
        let trimmed = token.trim();
        if trimmed.len() > 0 {
            let value = trimmed.parse_i64().unwrap();
            total = total + value;
        }
    }
    return total;
}

pub fn csv_count(text: &String) -> i64 {
    let mut count = 0;
    for token in text.split(",") {
        let trimmed = token.trim();
        if trimmed.len() > 0 {
            count = count + 1;
        }
    }
    return count;
}

pub fn digit_total_text(text: &String) -> i64 {
    let mut total = 0;
    for ch in text.chars() {
        let code = ch as i64;
        if code >= 48 && code <= 57 {
            total = total + (code - 48);
        }
    }
    return total;
}

pub fn count_digits(text: &String) -> i64 {
    let mut count = 0;
    for ch in text.chars() {
        let code = ch as i64;
        if code >= 48 && code <= 57 {
            count = count + 1;
        }
    }
    return count;
}

pub fn decorate(prefix: &String, value: i64) -> String {
    return format!("{}-{}", prefix, value);
}

pub fn join_words(words: Vec<String>, separator: &String) -> String {
    let mut out = String::new();
    let mut first = true;
    for word in words {
        if first {
            first = false;
        } else {
            out = out + separator.as_str();
        }
        out = out + word.as_str();
    }
    return out;
}

pub fn first_letters(words: Vec<String>) -> String {
    let mut out = String::new();
    for word in words {
        if word.len() > 0 {
            out = out + word.substring(0, 1).as_str();
        }
    }
    return out;
}

pub fn longest_word(words: Vec<String>) -> i64 {
    let mut longest: i64 = 0;
    for word in words {
        if word.len() as i64 > longest {
            longest = word.len() as i64;
        }
    }
    return longest;
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;
    let sample = String::from("the quick brown fox jumps over the lazy dog");
    let lengths = word_lengths(&sample);
    acc = (acc * 31 + lengths.len() as i64) % MODULUS;
    let words = list_words(&sample);
    let total = word_total(words.clone());
    acc = (acc * 31 + total) % MODULUS;
    let doubled = repeat(&sample, 2);
    acc = (acc * 31 + doubled.len() as i64) % MODULUS;
    acc = (acc * 31 + shout(&sample).len() as i64) % MODULUS;
    acc = (acc * 31 + whisper(&sample).len() as i64) % MODULUS;
    let csv = String::from("1,2,3,4,5");
    acc = (acc * 31 + csv_total(&csv)) % MODULUS;
    let sparse = String::from("1,2,,3");
    acc = (acc * 31 + csv_count(&sparse)) % MODULUS;
    let noisy = String::from("room 101, floor 7");
    acc = (acc * 31 + digit_total_text(&noisy)) % MODULUS;
    acc = (acc * 31 + count_digits(&noisy)) % MODULUS;
    let prefix = String::from("id");
    acc = (acc * 31 + decorate(&prefix, 42).len() as i64) % MODULUS;
    let letters = first_letters(words.clone());
    acc = (acc * 31 + letters.len() as i64) % MODULUS;
    acc = (acc * 31 + longest_word(words.clone())) % MODULUS;
    let dash = String::from("-");
    let glued = join_words(words, &dash);
    acc = (acc * 31 + glued.len() as i64) % MODULUS;
    return acc;
}
