fn main() {
    let values = vec![1, 2, 3];
    println!("{}", values[0]);
    let part = &values[0..5];
    println!("{}", part.len());
}
