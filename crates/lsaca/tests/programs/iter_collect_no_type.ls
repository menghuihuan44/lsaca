fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3];
    let collected = numbers.iter().map(|n| n * 2).collect();
    println!("{:?}", collected);
}
