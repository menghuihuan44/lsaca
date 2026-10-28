// Cloning a Vec deep-copies its elements; the source keeps its own storage.
fn main() {
    let words = vec![String::from("alpha"), String::from("beta")];
    let copy = words.clone();
    println!("{} {}", words.len(), copy.len());
    println!("{:?}", words);

    let mut changed = copy.clone();
    changed.push(String::from("gamma"));
    changed[0] = String::from("changed");
    println!("{:?} {:?}", copy, changed);

    let mut nested = vec![vec![1, 2], vec![3]];
    let nested_copy = nested.clone();
    nested[0].push(9);
    println!("{:?} {:?}", nested, nested_copy);
}
