// A HashMap value cannot be a tuple that contains a reference (E0498).
fn main() {
    let text = String::from("name");
    let mut map: HashMap<String, (i64, &String)> = HashMap::new();
    map.insert(String::from("k"), (1, &text));
    println!("{}", map.len());
}
