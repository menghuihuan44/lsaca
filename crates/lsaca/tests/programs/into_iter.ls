// Vec::into_iter() consumes the vector and yields owned items.
fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4];
    let mut total: i64 = 0;
    for value in numbers.into_iter() {
        total += value;
    }
    println!("total {}", total);

    let lengths: Vec<usize> = vec![String::from("hi"), String::from("there")]
        .into_iter()
        .map(|word| word.len())
        .collect();
    println!("{:?}", lengths);

    let doubled: Vec<i64> = vec![2i64, 4, 6].into_iter().map(|value| value * 3).collect();
    println!("{:?}", doubled);
}
