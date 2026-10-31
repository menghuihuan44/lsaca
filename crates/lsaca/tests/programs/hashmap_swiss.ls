fn main() {
    let mut values: HashMap<i64, i64> = HashMap::new();
    let mut index = 0;
    while index < 2000 {
        values.insert(index, index * 3);
        index = index + 1;
    }
    let mut sum = 0;
    let mut probe = 0;
    while probe < 2000 {
        if values.contains_key(probe) {
            sum = sum + values.get(probe).unwrap();
        }
        probe = probe + 1;
    }
    println!("{} {}", values.len(), sum);
    let mut removed = 0;
    let mut key = 1;
    while key < 2000 {
        if values.contains_key(key) {
            values.remove(key);
            removed = removed + 1;
        }
        key = key + 2;
    }
    println!("{} {}", removed, values.len());
    let mut kept = 0;
    let mut check = 0;
    while check < 2000 {
        if values.contains_key(check) {
            kept = kept + 1;
        }
        check = check + 1;
    }
    println!("{}", kept);
}
