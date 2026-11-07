// Array lengths must be const evaluated to a non-negative integer (E0435).
fn main() {
    let count: usize = 3;
    let values: [i64; count] = [1, 2, 3];
    println!("{}", values[0]);
}
