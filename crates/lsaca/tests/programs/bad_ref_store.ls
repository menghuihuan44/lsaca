// Storing a reference inside a Vec is still rejected (v2.2 Phase C).
fn main() {
    let number = 1;
    let values: Vec<&i64> = vec![&number];
    println!("{}", values.len());
}
