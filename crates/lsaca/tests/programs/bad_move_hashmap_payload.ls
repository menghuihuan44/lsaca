// A HashMap is never Copy: moving one out of a struct field is E0507.
struct Registry {
    entries: HashMap<String, i64>,
}

fn main() {
    let mut entries: HashMap<String, i64> = HashMap::new();
    entries.insert(String::from("a"), 1);
    let registry = Registry { entries: entries };
    let moved = registry.entries;
    println!("{}", moved.len());
}
