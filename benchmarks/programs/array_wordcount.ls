// array_wordcount: count the same deterministic word stream as
// hashmap_wordcount in a flat array indexed by the word number (the word space
// is fixed at 4096), then print the same frequency checksum. The alternative
// trade-off: direct indexing instead of a hash map.
// Optional argument: token count (default 200,000, the medium scale).
const DISTINCT: i64 = 4096;

fn main() {
    let all = args();
    let tokens: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 200000 };
    let mut counts: Vec<i64> = Vec::new();
    let mut i: i64 = 0;
    while i < DISTINCT {
        counts.push(0);
        i = i + 1;
    }
    let mut token: i64 = 0;
    while token < tokens {
        let index = token % DISTINCT;
        let current = counts[index];
        counts[index] = current + 1;
        token = token + 1;
    }
    let mut checksum: i64 = 0;
    let mut max_count: i64 = 0;
    let mut k: i64 = 0;
    while k < DISTINCT {
        let count = counts[k];
        checksum = checksum + count * count;
        if count > max_count {
            max_count = count;
        }
        k = k + 1;
    }
    println!("checksum={},{},{}", checksum, DISTINCT, max_count);
}