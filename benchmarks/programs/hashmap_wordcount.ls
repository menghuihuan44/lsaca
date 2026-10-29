// hashmap_wordcount: count 200,000 deterministic words (4,096 distinct) with
// the builtin HashMap<String, i64>, then print a checksum over the frequencies,
// the number of distinct keys and the highest frequency. The checksum (sum of
// squared counts) is independent of hash-map iteration order.
fn main() {
    let tokens = 200000;
    let distinct = 4096;
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut i = 0;
    while i < tokens {
        let word = format!("w{}", i % distinct);
        let next = match counts.get(word) {
            Some(count) => count + 1,
            None => 1,
        };
        counts.insert(word, next);
        i += 1;
    }
    let mut checksum = 0;
    let mut max_count = 0;
    let keys = counts.keys();
    for key in keys {
        let count = counts.get(key).unwrap();
        checksum += count * count;
        if count > max_count {
            max_count = count;
        }
    }
    println!("checksum={},{},{}", checksum, counts.len(), max_count);
}
