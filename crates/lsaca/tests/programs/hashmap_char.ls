fn main() {
    let mut counts: HashMap<char, i64> = HashMap::new();
    let words = "hello";
    let chars = words.chars();
    for index in 0..chars.len() {
        let letter = chars.get(index).unwrap();
        if counts.contains_key(letter) {
            let previous = counts.get(letter).unwrap();
            counts.insert(letter, previous + 1);
        } else {
            counts.insert(letter, 1);
        }
    }
    let mut total = 0;
    let probes = vec!['h', 'e', 'l', 'o', 'z'];
    for index in 0..probes.len() {
        let probe = probes[index];
        if counts.contains_key(probe) {
            total = total + counts.get(probe).unwrap();
        }
    }
    println!("{} {}", counts.len(), total);
    let removed = counts.remove('l');
    println!("{:?} {}", removed, counts.len());
    counts.clear();
    println!("{} {}", counts.len(), counts.is_empty());
}
