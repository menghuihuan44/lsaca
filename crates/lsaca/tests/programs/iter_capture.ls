fn main() {
    let suffix = String::from("!");
    let factor = 3;
    let numbers: Vec<i64> = vec![1, 2, 3];
    let shouted: Vec<String> = numbers.iter().map(|n| format!("{}{}", n * factor, suffix)).collect();
    println!("{:?}", shouted);

    let base = 10;
    let total: i64 = vec![1, 2, 3].iter().fold(0, |acc, item| acc + item * base);
    println!("total {}", total);
}
