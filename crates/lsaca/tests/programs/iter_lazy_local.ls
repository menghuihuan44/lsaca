fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4, 5, 6];
    let mapped = numbers.iter().map(|n| n * 2);
    let kept = mapped.filter(|n| *n > 6);
    for value in kept {
        println!("{}", value);
    }
}
