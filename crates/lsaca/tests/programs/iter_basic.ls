fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4];
    let mut total: i64 = 0;
    for value in numbers.iter() {
        total = total + value;
    }
    println!("total {}", total);
}
