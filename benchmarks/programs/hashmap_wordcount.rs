// hashmap_wordcount: count deterministic words (4,096 distinct) with
// std::collections::HashMap and String keys, then print a checksum over the
// frequencies, the number of distinct keys and the highest frequency. The
// checksum (sum of squared counts) is independent of hash-map iteration order.
// Optional argument: token count (default 200,000, the medium scale).
use std::collections::HashMap;

fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tokens = arg_i64(&args, 0, 200_000);
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
