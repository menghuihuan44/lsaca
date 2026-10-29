// get_with maps the stored value through a closure; the result must be an
// owned value. Returning a reference would let it escape the map and dangle
// after a mutation, so the closure result type is checked by the type checker.
fn main() {
    let mut map: HashMap<String, String> = HashMap::new();
    map.insert(String::from("a"), String::from("value"));
    let got = map.get_with(String::from("a"), |value: &String| -> &String { value });
    println!("{:?}", got);
}
