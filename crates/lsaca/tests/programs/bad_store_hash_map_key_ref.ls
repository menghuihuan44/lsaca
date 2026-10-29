// A reference cannot be stored as a HashMap key (E0498).
fn main() {
    let text = String::from("key");
    let mut map: HashMap<&String, i64> = HashMap::new();
    map.insert(&text, 1);
    println!("{}", map.len());
}
