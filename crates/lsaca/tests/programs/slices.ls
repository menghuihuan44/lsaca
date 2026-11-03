fn total(values: &[i64]) -> i64 {
    let mut sum = 0;
    let mut index = 0;
    while index < values.len() {
        sum = sum + values[index];
        index = index + 1;
    }
    return sum;
}

fn bump(values: &mut [i64]) {
    let mut index = 0;
    while index < values.len() {
        values[index] = values[index] + 1;
        index = index + 1;
    }
}

fn main() {
    let values = vec![10, 20, 30, 40, 50];
    println!("{}", total(&values[1..4]));
    println!("{}", total(&values[..]));
    println!("{}", total(&values[2..]));
    println!("{}", total(&values[..=1]));

    let mut numbers = vec![1, 2, 3];
    bump(&mut numbers[1..]);
    println!("{:?}", numbers);

    let words = vec![String::from("alpha"), String::from("beta")];
    let part: &[String] = &words;
    println!("{} {}", part.len(), words[0].len());
}
