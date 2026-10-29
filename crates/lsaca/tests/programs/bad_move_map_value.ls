// Moving a Vec value into a HashMap leaves it moved: the value argument of
// insert is consumed, so using it again is E0382.
fn main() {
    let mut groups: HashMap<String, Vec<i64>> = HashMap::new();
    let values: Vec<i64> = vec![1, 2, 3];
    groups.insert(String::from("a"), values);
    println!("{:?}", values);
}
