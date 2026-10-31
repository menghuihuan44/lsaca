fn mix(fold: i64, value: i64) -> i64 {
    (fold * 131 + value) % 1000000007
}

fn key_at(i: i64) -> String {
    format!("k{}-{}", i % 1000, i)
}

fn main() {
    let tokens = 200000;
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut i = 0;
    while i < tokens {
        counts.insert(key_at(i), (i % 10007) + 1);
        i += 1;
    }
    let mut fold = 0;
    let mut hits = 0;
    i = 0;
    while i < tokens {
        let value = counts.get(key_at(i)).unwrap();
        hits += 1;
        fold = mix(fold, value);
        i += 1;
    }
    i = 0;
    while i < tokens {
        let absent = key_at(tokens + i);
        if counts.contains_key(absent) {
            fold = mix(fold, counts.get(absent).unwrap());
        } else {
            let chars = absent.chars();
            let mut index = 0;
            while index < chars.len() {
                fold = mix(fold, chars.get(index).unwrap() as i64);
                index += 1;
            }
        }
        i += 1;
    }
    let mut d = 0;
    while d < 20000 {
        match counts.remove(key_at(d * 10)) {
            Some(value) => {
                fold = mix(fold, value);
            }
            None => {
                fold = mix(fold, 1);
            }
        }
        d += 1;
    }
    println!("checksum={},{},{}", fold, counts.len(), hits);
}
