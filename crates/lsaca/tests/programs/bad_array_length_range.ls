// Array lengths are capped at 1048576 (E0080).
fn main() {
    let values: [i64; 2000000] = [1, 2];
    println!("{}", values[0]);
}
