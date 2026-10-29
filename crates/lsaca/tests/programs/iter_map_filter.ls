fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4, 5];
    for value in numbers.iter().map(|n| n * 3).filter(|n| *n % 2 == 0) {
        println!("{}", value);
    }
}
