// HashMap::from(Vec<(K, V)>) inserts owned pairs in order; a repeated key keeps
// the last value. HashSet::from(Vec<K>) is the matching set constructor.
fn main() {
    let table: HashMap<String, i64> = HashMap::from(vec![
        (String::from("one"), 1),
        (String::from("two"), 2),
        (String::from("two"), 22),
    ]);
    println!("{} {} {}", table.len(), table.get(&String::from("two")).unwrap(), table.keys().count());

    let numbers: HashMap<i64, i64> = HashMap::from(vec![(1i64, 10i64), (2i64, 20i64)]);
    let mut total: i64 = 0;
    for (key, value) in &numbers {
        total += *key + *value;
    }
    println!("total {}", total);

    let set: HashSet<i64> = HashSet::from(vec![4i64, 4i64, 2i64, 7i64]);
    println!("{} {:?}", set.len(), set.elements());
}
