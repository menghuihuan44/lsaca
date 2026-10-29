// A reference cannot be stored as a Vec element (E0498).
fn main() {
    let number = 1;
    let values: Vec<&i64> = vec![&number];
    println!("{}", values.len());
}
