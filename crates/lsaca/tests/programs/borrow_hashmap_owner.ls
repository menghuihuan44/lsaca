// HashMap borrow rules: keys are copied out of the caller, get_with only
// borrows the map for the call, and the map can move after the borrow ends.
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    let key = String::from("a");
    counts.insert(key, 7);

    let label = String::from("a");
    let shown = counts.get_with(label, |value: &i64| -> String { format!("v={}", value) });
    println!("{}", shown.unwrap());

    // get_with copied its key instead of moving it.
    println!("{}", label);

    // The borrow of counts ended with the call, so the map can move now.
    let moved = counts;
    println!("{}", moved.len());
}