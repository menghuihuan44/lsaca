fn sum(values: &[i64]) -> i64 {
    let mut total: i64 = 0;
    let mut index: usize = 0;
    while index < values.len() {
        total = total + values[index];
        index = index + 1;
    }
    total
}

fn main() {
    let numbers: [i64; 5] = [10, 20, 30, 40, 50];
    println!("{} {}", numbers.len(), numbers[2]);
    let zeros: [i64; 4] = [0; 4];
    println!("{:?}", zeros);
    println!("{}", sum(&numbers));
    let first_three: &[i64] = &numbers[0..3];
    println!("{:?}", first_three);
    let empty: [i64; 0] = [];
    println!("{} {}", empty.len(), empty.is_empty());
    let same: [i64; 5] = [10, 20, 30, 40, 50];
    println!("{}", numbers == same);
    let other: [i64; 5] = [1, 2, 3, 4, 5];
    println!("{}", numbers != other);
}
