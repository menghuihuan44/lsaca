fn mix(fold: i64, value: i64) -> i64 {
    (fold * 131 + value) % 1000000007
}

fn key_at(i: i64) -> i64 {
    (i * 2654435761) % 2147483648
}

fn main() {
    let n = 1000000;
    let mut values: HashMap<i64, i64> = HashMap::new();
    let mut i = 0;
    while i < n {
        let key = key_at(i);
        values.insert(key, (key % 10007) + 1);
        i += 1;
    }
    let mut fold = 0;
    let mut hits = 0;
    i = 0;
    while i < n {
        let value = values.get(key_at(i)).unwrap();
        hits += 1;
        fold = mix(fold, value);
        i += 1;
    }
    i = 0;
    while i < n {
        let absent = 2147483648 + i;
        if values.contains_key(absent) {
            fold = mix(fold, values.get(absent).unwrap());
        } else {
            fold = mix(fold, absent % 97);
        }
        i += 1;
    }
    let mut d = 0;
    while d < 100000 {
        match values.remove(key_at(d * 10)) {
            Some(value) => {
                fold = mix(fold, value);
            }
            None => {
                fold = mix(fold, 1);
            }
        }
        d += 1;
    }
    println!("checksum={},{},{}", fold, values.len(), hits);
}
