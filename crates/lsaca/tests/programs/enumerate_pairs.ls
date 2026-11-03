fn main() {
    let values: Vec<i64> = vec![4, 5, 6];
    let mut total = 0;
    for pair in values.clone().iter().enumerate() {
        let (index, value) = pair;
        total += index * value;
    }
    println!("{}", total);
    let mut weighted = 0;
    for (index, value) in values.clone().iter().enumerate() {
        weighted += index + value;
    }
    println!("{}", weighted);
    let adjusted: i64 = values
        .iter()
        .enumerate()
        .map(|pair| pair.1 - pair.0)
        .fold(0, |acc, value| acc + value);
    println!("{}", adjusted);
}
