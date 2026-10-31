// hashmap_string: 200,000 distinct generated String keys inserted into std
// HashMap, then 200,000 present lookups, 200,000 absent lookups and 20,000
// deletions; prints the folded checksum, final size and successful lookups.
use std::collections::HashMap;

const MOD: i64 = 1000000007;

fn mix(fold: i64, value: i64) -> i64 {
    (fold * 131 + value) % MOD
}

fn key_at(i: i64) -> String {
    format!("k{}-{}", i % 1000, i)
}

fn main() {
    let tokens: i64 = 200000;
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut i = 0;
    while i < tokens {
        counts.insert(key_at(i), (i % 10007) + 1);
        i += 1;
    }
    let mut fold: i64 = 0;
    let mut hits: i64 = 0;
    i = 0;
    while i < tokens {
        fold = mix(fold, *counts.get(&key_at(i)).unwrap());
        hits += 1;
        i += 1;
    }
    i = 0;
    while i < tokens {
        let absent = key_at(tokens + i);
        match counts.get(&absent) {
            Some(value) => fold = mix(fold, *value),
            None => {
                for byte in absent.as_bytes() {
                    fold = mix(fold, *byte as i64);
                }
            }
        }
        i += 1;
    }
    let mut d = 0;
    while d < 20000 {
        match counts.remove(&key_at(d * 10)) {
            Some(value) => fold = mix(fold, value),
            None => fold = mix(fold, 1),
        }
        d += 1;
    }
    println!("checksum={},{},{}", fold, counts.len(), hits);
}
