// HashMap: get_with, keys/values, content equality and the other key types.
fn main() {
    let mut map: HashMap<String, i64> = HashMap::new();
    map.insert(String::from("x"), 10);
    map.insert(String::from("y"), 20);

    let shown = map.get_with(String::from("x"), |value| format!("x={}", value));
    println!("{}", shown.unwrap());
    let missing = map.get_with(String::from("z"), |value| format!("never {}", value));
    println!("{:?}", missing);

    let keys = map.keys();
    println!("{}", keys.len());
    let mut key_chars = 0;
    for key in keys {
        key_chars = key_chars + key.len();
    }
    println!("{}", key_chars);

    let values = map.values();
    let mut total = 0;
    for value in values {
        total = total + value;
    }
    println!("{}", total);

    let mut other: HashMap<String, i64> = HashMap::new();
    other.insert(String::from("y"), 20);
    other.insert(String::from("x"), 10);
    println!("{} {}", map == other, map == HashMap::new());

    let mut flags: HashMap<bool, u64> = HashMap::new();
    flags.insert(true, 1 as u64);
    flags.insert(false, 2 as u64);
    println!("{} {}", flags.get(true).unwrap(), flags.get(false).unwrap());
}
