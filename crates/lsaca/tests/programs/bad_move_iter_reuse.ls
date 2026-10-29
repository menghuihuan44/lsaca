// Vec::iter() consumes the vector: the vector is moved into the iterator, so
// using it again after the for loop is E0382.
fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3];
    for value in numbers.iter() {
        println!("{}", value);
    }
    println!("{}", numbers.len());
}
