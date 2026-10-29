fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4, 5, 6];
    let limit = 3;
    for pair in numbers.iter().map(|n| n * n).filter(|n| *n > 10).take(limit).enumerate() {
        let (index, value) = pair;
        println!("{} {}", index, value);
    }
}
