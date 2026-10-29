// Builtin HashMap: String keys, insert/replace, lookups, remove, len.
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 1);
    counts.insert(String::from("b"), 2);
    counts.insert(String::from("a"), 3);
    println!("{} {}", counts.len(), counts.get(String::from("a")).unwrap());
    if counts.contains_key(String::from("b")) {
        println!("has b");
    }
    let removed = counts.remove(String::from("a"));
    println!("{:?}", removed);
    println!("{} {}", counts.len(), counts.contains_key(String::from("a")));

    let mut scalars: HashMap<i64, bool> = HashMap::new();
    scalars.insert(7, true);
    scalars.insert(8, false);
    println!("{} {}", scalars.get(7).unwrap(), scalars.get(8).unwrap());
    println!("{}", scalars.is_empty());
    scalars.clear();
    println!("{} {}", scalars.is_empty(), scalars.len());
}
