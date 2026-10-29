// hashmap_wordcount: count 200,000 deterministic words (4,096 distinct) with
// std::collections::HashMap and String keys, then print a checksum over the
// frequencies, the number of distinct keys and the highest frequency. The
// checksum (sum of squared counts) is independent of hash-map iteration order.
use std::collections::HashMap;

fn main() {
    let tokens: i64 = 200_000;
    let distinct: i64 = 4_096;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for i in 0..tokens {
        let word = format!("w{}", i % distinct);
        *counts.entry(word).or_insert(0) += 1;
    }
    let mut checksum: i64 = 0;
    let mut max_count: i64 = 0;
    for count in counts.values() {
        checksum += count * count;
        if *count > max_count {
            max_count = *count;
        }
    }
    println!("checksum={},{},{}", checksum, counts.len(), max_count);
}
