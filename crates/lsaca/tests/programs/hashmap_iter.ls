// Borrowed HashMap iteration: pairs, keys and values all yield references and
// never clone. The sums are independent of the hash-map slot order.
fn main() {
    let mut table: HashMap<String, i64> = HashMap::new();
    table.insert(String::from("a"), 1);
    table.insert(String::from("bb"), 2);
    table.insert(String::from("ccc"), 3);

    let mut pairs: i64 = 0;
    for (key, value) in &table {
        pairs += (key.len() as i64) * *value;
    }
    println!("pairs {}", pairs);

    let mut key_chars: i64 = 0;
    for key in table.keys() {
        key_chars += key.len() as i64;
    }
    let mut value_total: i64 = 0;
    for value in table.values() {
        value_total += *value;
    }
    println!("{} {} {}", key_chars, value_total, table.len());

    let mut explicit: i64 = 0;
    for (key, value) in table.iter() {
        explicit += (*value) * 2 + (key.len() as i64);
    }
    println!("explicit {}", explicit);
}
