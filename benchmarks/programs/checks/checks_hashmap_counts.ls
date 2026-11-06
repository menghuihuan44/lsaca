// Property check: HashMap<String, i64> frequencies agree with a flat indexed
// array, the key count and total token count are exact, and the sum of
// squared counts does not depend on the hash-map iteration order.
fn main() {
    let distinct: i64 = 256;
    let tokens: i64 = 10000;
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut array: Vec<i64> = Vec::new();
    let mut index: i64 = 0;
    while index < distinct {
        array.push(0);
        index += 1;
    }
    let mut seed: i64 = 99;
    let mut position: i64 = 0;
    while position < tokens {
        seed = (seed * 1103515245 + 12345) % 2147483648;
        let bucket = seed % distinct;
        let word = format!("w{}", bucket);
        let next = match counts.get(word) {
            Some(count) => count + 1,
            None => 1,
        };
        counts.insert(word, next);
        array[bucket as usize] = array[bucket as usize] + 1;
        position += 1;
    }
    assert_eq!(counts.len(), distinct as usize);
    let mut total: i64 = 0;
    let mut bucket: i64 = 0;
    while bucket < distinct {
        total += array[bucket as usize];
        bucket += 1;
    }
    assert_eq!(total, tokens);
    let mut counted: i64 = 0;
    let keys = counts.keys();
    for key in keys {
        let value = counts.get(key).unwrap();
        counted += value;
    }
    assert_eq!(counted, tokens);
    let mut checksum_map: i64 = 0;
    let keys_again = counts.keys();
    for key in keys_again {
        let value = counts.get(key).unwrap();
        checksum_map += value * value;
    }
    let mut checksum_array: i64 = 0;
    let mut position_again: i64 = 0;
    while position_again < distinct {
        let value = array[position_again as usize];
        checksum_array += value * value;
        position_again += 1;
    }
    assert_eq!(checksum_map, checksum_array);
    println!("ok hashmap");
}
