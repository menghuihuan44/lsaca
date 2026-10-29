// Moving a String key into a HashMap leaves it moved: the key argument of
// insert is consumed, so using it again is E0382.
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    let key = String::from("a");
    counts.insert(key, 1);
    println!("{}", key);
}
