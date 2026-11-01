// table: ordered word counters for the util_b library package.

use util_a::hash_text;

pub const MODULUS: i64 = 1000000007;

pub struct Entry {
    pub word: String,
    pub count: i64,
}

pub struct Counter {
    pub counts: HashMap<String, i64>,
    pub order: Vec<String>,
}

impl Counter {
    pub fn new() -> Counter {
        return Counter { counts: HashMap::new(), order: Vec::new() };
    }

    pub fn touch(&mut self, word: String) {
        let existing = self.counts.get(word);
        let next = match existing {
            Some(count) => count + 1,
            None => 1,
        };
        if next == 1 {
            self.order.push(word.clone());
        }
        self.counts.insert(word, next);
    }

    pub fn entry_count(&self, word: &String) -> i64 {
        let key = word.clone();
        let existing = self.counts.get(key);
        match existing {
            Some(count) => {
                return count;
            }
            None => {
                return 0;
            }
        }
    }

    pub fn distinct(&self) -> i64 {
        return self.counts.len();
    }

    pub fn entries(&self) -> Vec<Entry> {
        let mut result: Vec<Entry> = Vec::new();
        let mut index = 0;
        let len = self.order.len();
        while index < len {
            let word = self.order[index].clone();
            let count = self.entry_count(&word);
            result.push(Entry { word: word, count: count });
            index = index + 1;
        }
        return result;
    }

    pub fn checksum(&self) -> i64 {
        let mut acc = 0;
        let entries = self.entries();
        for entry in entries {
            let signature = entry_signature(&entry);
            acc = (acc * 31 + signature) % MODULUS;
        }
        return acc;
    }
}

pub fn entry_signature(entry: &Entry) -> i64 {
    let mut state = hash_text(&entry.word);
    state = (state * 31 + entry.count) % MODULUS;
    return state;
}

pub fn word_count(words: Vec<String>, limit: i64) -> Counter {
    let mut counter = Counter::new();
    for word in words {
        if word.len() > 0 && word.len() <= limit {
            counter.touch(word);
        }
    }
    return counter;
}

pub fn total_occurrences(counter: &Counter) -> i64 {
    let mut total = 0;
    let mut index = 0;
    let len = counter.order.len();
    while index < len {
        let word = counter.order[index].clone();
        total = total + counter.entry_count(&word);
        index = index + 1;
    }
    return total;
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;
    let mut words: Vec<String> = Vec::new();
    words.push(String::from("alpha"));
    words.push(String::from("beta"));
    words.push(String::from("alpha"));
    words.push(String::from("gamma"));
    words.push(String::from("alpha"));
    words.push(String::from("delta"));
    let counter = word_count(words, 8);
    acc = (acc * 31 + counter.distinct()) % MODULUS;
    acc = (acc * 31 + counter.checksum()) % MODULUS;
    acc = (acc * 31 + total_occurrences(&counter)) % MODULUS;
    let alpha = String::from("alpha");
    acc = (acc * 31 + counter.entry_count(&alpha)) % MODULUS;
    let missing = String::from("omega");
    acc = (acc * 31 + counter.entry_count(&missing)) % MODULUS;
    let entries = counter.entries();
    let mut weighted = 0;
    let mut index = 0;
    while index < entries.len() {
        let entry = &entries[index];
        weighted = (weighted * 31 + entry_signature(entry)) % MODULUS;
        index = index + 1;
    }
    acc = (acc * 31 + weighted) % MODULUS;
    return acc;
}
