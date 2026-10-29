// .zip() is not part of the supported iterator surface: it must be rejected
// with a diagnostic that does not leak internal prelude identifiers.
fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let zipped = values.iter().zip(vec![4, 5, 6].iter());
    println!("{}", zipped.count());
}
