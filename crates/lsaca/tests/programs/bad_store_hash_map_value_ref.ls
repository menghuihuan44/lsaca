// A reference cannot be stored as a HashMap value (E0498).
fn main() {
    let number = 1;
    let mut map: HashMap<String, &i64> = HashMap::new();
    map.insert(String::from("k"), &number);
    println!("{}", map.len());
}
