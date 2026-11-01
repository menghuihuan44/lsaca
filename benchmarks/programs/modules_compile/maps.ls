// maps: builtin HashMap workloads over owned keys and integer values.
// Part of the modules_compile compile-time workload.
//
// Key sets are built deterministically. All checksums are order independent:
// they fold over the keys() vector, so hash-map iteration order cannot affect
// the result.

pub const MODULUS: i64 = 1000000007;

pub struct Counter {
    counts: HashMap<String, i64>,
    total: i64,
}

impl Counter {
    pub fn new() -> Counter {
        Counter {
            counts: HashMap::new(),
            total: 0,
        }
    }

    pub fn add(&mut self, word: &String) -> i64 {
        let next = match self.counts.get(word.clone()) {
            Some(count) => count + 1,
            None => 1,
        };
        self.counts.insert(format!("{}", word), next);
        self.total += 1;
        next
    }

    pub fn add_owned(&mut self, word: String) -> i64 {
        let next = match self.counts.get(word.clone()) {
            Some(count) => count + 1,
            None => 1,
        };
        self.counts.insert(word, next);
        self.total += 1;
        next
    }

    pub fn count_of(&self, word: &String) -> i64 {
        match self.counts.get(word.clone()) {
            Some(count) => count,
            None => 0,
        }
    }

    pub fn total(&self) -> i64 {
        self.total
    }

    pub fn distinct(&self) -> i64 {
        self.counts.len()
    }

    pub fn most_common_count(&self) -> i64 {
        let mut best = 0;
        let keys = self.counts.keys();
        for key in keys {
            let count = self.counts.get(key).unwrap();
            if count > best {
                best = count;
            }
        }
        best
    }

    pub fn most_common_key(&self) -> String {
        let mut best = 0;
        let mut chosen = String::from("");
        let keys = self.counts.keys();
        for key in keys {
            let count = self.counts.get(key).unwrap();
            if count > best {
                best = count;
                chosen = format!("{}", key);
            }
        }
        chosen
    }

    pub fn keys_above(&self, limit: i64) -> i64 {
        let mut count = 0;
        let keys = self.counts.keys();
        for key in keys {
            let value = self.counts.get(key).unwrap();
            if value > limit {
                count += 1;
            }
        }
        count
    }

    pub fn frequency_squares(&self) -> i64 {
        let mut total = 0;
        let keys = self.counts.keys();
        for key in keys {
            let count = self.counts.get(key).unwrap();
            total += count * count;
        }
        total
    }

    pub fn key_length_total(&self) -> i64 {
        let mut total = 0;
        let keys = self.counts.keys();
        for key in keys {
            total += key.len();
        }
        total
    }

    pub fn checksum(&self) -> i64 {
        let mut total = 0;
        let keys = self.counts.keys();
        for key in keys {
            let count = self.counts.get(key).unwrap();
            total += count * 31 + key.len();
        }
        total % MODULUS
    }
}

pub fn word_frequencies(words: Vec<String>) -> HashMap<String, i64> {
    let mut counts: HashMap<String, i64> = HashMap::new();
    for word in words {
        let next = match counts.get(word) {
            Some(count) => count + 1,
            None => 1,
        };
        counts.insert(word, next);
    }
    counts
}

pub fn digit_frequencies(numbers: Vec<i64>) -> HashMap<i64, i64> {
    let mut counts: HashMap<i64, i64> = HashMap::new();
    for number in numbers {
        let key = number % 10;
        let next = match counts.get(key) {
            Some(count) => count + 1,
            None => 1,
        };
        counts.insert(key, next);
    }
    counts
}

pub fn bucket_frequencies(limit: i64, modulus: i64) -> HashMap<i64, i64> {
    let mut counts: HashMap<i64, i64> = HashMap::new();
    let mut value = 0;
    while value < limit {
        let key = (value * value) % modulus;
        let next = match counts.get(key) {
            Some(count) => count + 1,
            None => 1,
        };
        counts.insert(key, next);
        value += 1;
    }
    counts
}

pub fn build_lookup(entries: i64) -> HashMap<String, i64> {
    let mut table: HashMap<String, i64> = HashMap::new();
    let mut index = 0;
    while index < entries {
        table.insert(format!("key-{}", index), index * 7 % 23);
        index += 1;
    }
    table
}

pub fn lookup_or(table: &HashMap<String, i64>, key: &String, fallback: i64) -> i64 {
    match table.get(key.clone()) {
        Some(value) => value,
        None => fallback,
    }
}

pub fn lookup_total(table: &HashMap<String, i64>) -> i64 {
    let mut total = 0;
    let keys = table.keys();
    for key in keys {
        let value = table.get(key).unwrap();
        total += value;
    }
    total
}

pub fn lookup_squares(table: &HashMap<String, i64>) -> i64 {
    let mut total = 0;
    let keys = table.keys();
    for key in keys {
        let value = table.get(key).unwrap();
        total += value * value;
    }
    total
}

pub fn lookup_checksum(table: &HashMap<String, i64>) -> i64 {
    let mut total = 0;
    let keys = table.keys();
    for key in keys {
        let value = table.get(key).unwrap();
        total += value * 31 + key.len();
    }
    total % MODULUS
}

pub fn int_table_checksum(table: &HashMap<i64, i64>) -> i64 {
    let mut total = 0;
    let keys = table.keys();
    for key in keys {
        let value = table.get(key).unwrap();
        total += key * 31 + value;
    }
    total % MODULUS
}

pub fn int_table_total(table: &HashMap<i64, i64>) -> i64 {
    let mut total = 0;
    let keys = table.keys();
    for key in keys {
        let value = table.get(key).unwrap();
        total += value;
    }
    total
}

pub fn int_table_distinct(table: &HashMap<i64, i64>) -> i64 {
    table.len()
}

pub fn string_table(entries: i64) -> HashMap<String, String> {
    let mut table: HashMap<String, String> = HashMap::new();
    let mut index = 0;
    while index < entries {
        table.insert(
            format!("id-{}", index),
            format!("name-{}", index * 3 % 11),
        );
        index += 1;
    }
    table
}

pub fn string_table_key_length(table: &HashMap<String, String>) -> i64 {
    let mut total = 0;
    let keys = table.keys();
    for key in keys {
        total += key.len();
    }
    total
}

pub fn table_size(table: &HashMap<i64, i64>) -> i64 {
    table.len()
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;

    let words: Vec<String> = vec![
        String::from("red"),
        String::from("blue"),
        String::from("red"),
        String::from("green"),
        String::from("red"),
        String::from("blue"),
    ];
    let frequencies = word_frequencies(words);
    acc = (acc * 31 + frequencies.len()) % MODULUS;
    let red = String::from("red");
    acc = (acc * 31 + lookup_or(&frequencies, &red, 0)) % MODULUS;

    let missing = String::from("violet");
    acc = (acc * 31 + lookup_or(&frequencies, &missing, 9)) % MODULUS;

    let mut counter = Counter::new();
    let word = String::from("red");
    acc = (acc * 31 + counter.add(&word)) % MODULUS;
    acc = (acc * 31 + counter.add(&word)) % MODULUS;
    let extra = String::from("green");
    acc = (acc * 31 + counter.add(&extra)) % MODULUS;
    acc = (acc * 31 + counter.add_owned(String::from("green"))) % MODULUS;
    acc = (acc * 31 + counter.count_of(&word)) % MODULUS;
    acc = (acc * 31 + counter.total()) % MODULUS;
    acc = (acc * 31 + counter.distinct()) % MODULUS;
    acc = (acc * 31 + counter.most_common_count()) % MODULUS;
    acc = (acc * 31 + counter.most_common_key().len()) % MODULUS;
    acc = (acc * 31 + counter.keys_above(1)) % MODULUS;
    acc = (acc * 31 + counter.frequency_squares()) % MODULUS;
    acc = (acc * 31 + counter.key_length_total()) % MODULUS;
    acc = (acc * 31 + counter.checksum()) % MODULUS;

    let numbers: Vec<i64> = vec![11, 22, 33, 44, 55, 66, 77, 88, 99];
    let digits = digit_frequencies(numbers);
    acc = (acc * 31 + int_table_distinct(&digits)) % MODULUS;
    acc = (acc * 31 + int_table_total(&digits)) % MODULUS;
    acc = (acc * 31 + int_table_checksum(&digits)) % MODULUS;

    let buckets = bucket_frequencies(200, 17);
    acc = (acc * 31 + table_size(&buckets)) % MODULUS;
    acc = (acc * 31 + int_table_total(&buckets)) % MODULUS;
    acc = (acc * 31 + int_table_checksum(&buckets)) % MODULUS;

    let table = build_lookup(40);
    acc = (acc * 31 + table.len()) % MODULUS;
    acc = (acc * 31 + lookup_total(&table)) % MODULUS;
    acc = (acc * 31 + lookup_squares(&table)) % MODULUS;
    acc = (acc * 31 + lookup_checksum(&table)) % MODULUS;
    let known = String::from("key-7");
    acc = (acc * 31 + lookup_or(&table, &known, -1)) % MODULUS;
    let unknown = String::from("key-999");
    acc = (acc * 31 + lookup_or(&table, &unknown, 5)) % MODULUS;

    let names = string_table(12);
    acc = (acc * 31 + names.len()) % MODULUS;
    acc = (acc * 31 + string_table_key_length(&names)) % MODULUS;

    acc % MODULUS
}