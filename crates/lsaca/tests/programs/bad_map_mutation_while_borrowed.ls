// A HashMap insert mutates the map, so it cannot run while the map is
// shared-borrowed.
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 1);
    let shared = &counts;
    counts.insert(String::from("b"), 2);
    println!("{}", shared.len());
}