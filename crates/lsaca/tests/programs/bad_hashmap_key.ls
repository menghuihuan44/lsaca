// HashMap keys must be i64, u64, bool or String (E0277).
fn main() {
    let mut map: HashMap<f64, i64> = HashMap::new();
    map.insert(1.5, 1);
    println!("{}", map.len());
}
