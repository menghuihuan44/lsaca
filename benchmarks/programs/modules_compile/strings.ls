// strings: String construction, slicing, parsing and character scanning.
//
// Part of the modules_compile compile-time workload. Text handling here avoids
// locale-dependent behavior: every string is ASCII, slicing bounds are byte
// offsets that fall on character boundaries, and all numbers are decimal i64.

pub const MODULUS: i64 = 1000000007;

// repeat_text: concatenate a borrowed string with itself.
pub fn repeat_text(text: &String, times: i64) -> String {
    let mut result = String::new();
    let mut round = 0;
    while round < times {
        // A top-level &String formats quoted (docs/language/12-references-and-borrowing.md);
        // binding the formatted piece keeps the original output byte for byte.
        let piece = format!("{}", text);
        result.push_str(&piece);
        round += 1;
    }
    result
}

// build_csv: "0,1,2,...,limit-1,".
pub fn build_csv(limit: i64) -> String {
    let mut result = String::new();
    let mut value = 0;
    while value < limit {
        let piece = format!("{},", value);
        result.push_str(&piece);
        value += 1;
    }
    result
}

// parse_csv_sum: parse every non-empty field of a comma separated list.
pub fn parse_csv_sum(text: &String) -> i64 {
    let parts = text.split(",");
    let mut total = 0;
    for part in parts {
        if part.len() > 0 {
            total += part.parse_i64().unwrap();
        }
    }
    total
}

pub fn parse_csv_count(text: &String) -> i64 {
    let parts = text.split(",");
    let mut count = 0;
    for part in parts {
        if part.len() > 0 {
            count += 1;
        }
    }
    count
}

// vowel_count: scan characters through the char iterator by index.
pub fn vowel_count(text: &String) -> i64 {
    let characters = text.chars();
    let mut count = 0;
    let mut index = 0;
    while index < characters.len() {
        let character = characters.get(index).unwrap();
        if character == 'a'
            || character == 'e'
            || character == 'i'
            || character == 'o'
            || character == 'u'
        {
            count += 1;
        }
        index += 1;
    }
    count
}

pub fn consonant_count(text: &String) -> i64 {
    let characters = text.chars();
    let mut count = 0;
    let mut index = 0;
    while index < characters.len() {
        let character = characters.get(index).unwrap();
        let is_vowel = character == 'a'
            || character == 'e'
            || character == 'i'
            || character == 'o'
            || character == 'u';
        let is_letter = character >= 'a' && character <= 'z';
        if is_letter && !is_vowel {
            count += 1;
        }
        index += 1;
    }
    count
}

// digit_total: sum the numeric value of every decimal digit in the text.
pub fn digit_total(text: &String) -> i64 {
    let characters = text.chars();
    let zero = '0' as i64;
    let mut total = 0;
    let mut index = 0;
    while index < characters.len() {
        let character = characters.get(index).unwrap();
        if character >= '0' && character <= '9' {
            total += character as i64 - zero;
        }
        index += 1;
    }
    total
}

pub fn digit_count(text: &String) -> i64 {
    let characters = text.chars();
    let mut count = 0;
    let mut index = 0;
    while index < characters.len() {
        let character = characters.get(index).unwrap();
        if character >= '0' && character <= '9' {
            count += 1;
        }
        index += 1;
    }
    count
}

// is_palindrome: compare characters from both ends.
pub fn is_palindrome(text: &String) -> bool {
    let characters = text.chars();
    if characters.len() == 0 {
        return true;
    }
    let mut left = 0;
    let mut right = characters.len() - 1;
    while left < right {
        let first = characters.get(left).unwrap();
        let second = characters.get(right).unwrap();
        if first != second {
            return false;
        }
        left += 1;
        right -= 1;
    }
    true
}

// reverse_text: build a new string with the characters reversed.
pub fn reverse_text(text: &String) -> String {
    let characters = text.chars();
    let mut result = String::new();
    let mut index = characters.len();
    while index > 0 {
        index -= 1;
        result.push(characters.get(index).unwrap());
    }
    result
}

// substring_copy: byte based substring with safe ASCII bounds.
pub fn substring_copy(text: &String, start: i64, length: i64) -> String {
    let slice = text.substring(start as usize, length as usize);
    let mut result = String::new();
    result.push_str(slice.as_str());
    result
}

// find_text: first byte offset where needle starts, or -1.
pub fn find_text(text: &String, needle: &String) -> i64 {
    let characters = text.chars();
    let wanted = needle.chars();
    if wanted.len() == 0 || wanted.len() > characters.len() {
        return -1;
    }
    let limit = characters.len() - wanted.len();
    let mut start = 0;
    while start <= limit {
        let mut matched = true;
        let mut offset = 0;
        while offset < wanted.len() {
            let left = characters.get(start + offset).unwrap();
            let right = wanted.get(offset).unwrap();
            if left != right {
                matched = false;
                offset = wanted.len();
            } else {
                offset += 1;
            }
        }
        if matched {
            return start as i64;
        }
        start += 1;
    }
    return -1;
}

// uppercase_length: case conversion allocates a new owned string.
pub fn uppercase_length(text: &String) -> i64 {
    let upper = text.to_uppercase();
    upper.len() as i64
}

pub fn lowercase_length(text: &String) -> i64 {
    let lower = text.to_lowercase();
    lower.len() as i64
}

pub fn uppercase_copy(text: &String) -> String {
    let upper = text.to_uppercase();
    let mut result = String::new();
    result.push_str(upper.as_str());
    result
}

// trim_copy: trimming returns a borrowed slice; copy it into a new String.
pub fn trim_copy(text: &String) -> String {
    let trimmed = text.trim();
    let mut result = String::new();
    result.push_str(trimmed);
    result
}

pub fn is_empty(text: &String) -> bool {
    text.is_empty()
}

// join_words: concatenate words with an explicit separator.
pub fn join_words(words: &Vec<String>, separator: &String) -> String {
    let mut result = String::new();
    let mut index = 0;
    while index < words.len() {
        result.push_str(words[index].as_str());
        if index + 1 < words.len() {
            result.push_str(separator);
        }
        index += 1;
    }
    result
}

pub fn word_lengths(words: &Vec<String>) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::with_capacity(words.len());
    let mut index = 0;
    while index < words.len() {
        result.push(words[index].len() as i64);
        index += 1;
    }
    result
}

pub fn total_length(words: &Vec<String>) -> i64 {
    let lengths = word_lengths(words);
    let mut total = 0;
    let mut index = 0;
    while index < lengths.len() {
        total += lengths[index];
        index += 1;
    }
    total
}

pub fn longest_length(words: &Vec<String>) -> i64 {
    let lengths = word_lengths(words);
    let mut best = 0;
    let mut index = 0;
    while index < lengths.len() {
        if lengths[index] > best {
            best = lengths[index];
        }
        index += 1;
    }
    best
}

pub fn count_byte(words: &Vec<String>, wanted_byte: i64) -> i64 {
    let mut count = 0;
    let mut index = 0;
    while index < words.len() {
        if words[index].len() as i64 == wanted_byte {
            count += 1;
        }
        index += 1;
    }
    count
}

// first_word: the first non-empty field of a space separated text.
pub fn first_word(text: &String) -> String {
    let parts = text.split(" ");
    let mut result = String::new();
    for part in parts {
        if part.len() > 0 {
            result.push_str(part.as_str());
            return result;
        }
    }
    result
}

// word_total: sum the lengths of all space separated words.
pub fn word_total(text: &String) -> i64 {
    let parts = text.split(" ");
    let mut total = 0;
    for part in parts {
        if part.len() > 0 {
            total += part.len();
        }
    }
    total as i64
}

pub fn word_count(text: &String) -> i64 {
    let parts = text.split(" ");
    let mut count = 0;
    for part in parts {
        if part.len() > 0 {
            count += 1;
        }
    }
    count
}

// mutate_string: push, pop and append through a mutable heap reference.
pub fn build_target(prefix: &String, limit: i64) -> String {
    let mut text = String::new();
    text.push_str(prefix);
    let mut value = 0;
    while value < limit {
        let piece = format!("{}", value);
        text.push_str(&piece);
        value += 1;
    }
    text
}

pub fn drop_last(text: &mut String) -> i64 {
    if text.is_empty() {
        return -1;
    }
    let last = text.pop().unwrap();
    last as i64
}

pub fn append_marker(text: &mut String, marker: &String) {
    text.push_str(marker);
}

pub fn char_sum(text: &String) -> i64 {
    let characters = text.chars();
    let mut total = 0;
    let mut index = 0;
    while index < characters.len() {
        let character = characters.get(index).unwrap();
        total += character as i64;
        index += 1;
    }
    total
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;

    let greeting = String::from("lsaca");
    acc = (acc * 31 + greeting.len() as i64) % MODULUS;

    let upper = uppercase_copy(&greeting);
    acc = (acc * 31 + upper.len() as i64) % MODULUS;

    let csv = build_csv(5);
    acc = (acc * 31 + csv.len() as i64) % MODULUS;
    acc = (acc * 31 + parse_csv_sum(&csv)) % MODULUS;
    acc = (acc * 31 + parse_csv_count(&csv)) % MODULUS;

    let repeated = repeat_text(&greeting, 3);
    acc = (acc * 31 + repeated.len() as i64) % MODULUS;

    let phrase = String::from("programming");
    acc = (acc * 31 + vowel_count(&phrase)) % MODULUS;
    acc = (acc * 31 + consonant_count(&phrase)) % MODULUS;
    acc = (acc * 31 + char_sum(&phrase) % 1000) % MODULUS;

    let word = String::from("racecar");
    let mirrored = is_palindrome(&word);
    if mirrored {
        acc = (acc * 31 + 1) % MODULUS;
    } else {
        acc = (acc * 31 + 2) % MODULUS;
    }

    let haystack = String::from("needle in a haystack");
    let needle = String::from("in");
    let offset = find_text(&haystack, &needle);
    acc = (acc * 31 + offset) % MODULUS;

    let digits = String::from("room 101, floor 7");
    acc = (acc * 31 + digit_total(&digits)) % MODULUS;
    acc = (acc * 31 + digit_count(&digits)) % MODULUS;

    acc % MODULUS
}
