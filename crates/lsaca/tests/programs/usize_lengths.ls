// Lengths and indices are usize (v2.1).
fn index_of(numbers: &Vec<i64>, target: i64) -> usize {
    let mut i: usize = 0;
    while i < numbers.len() {
        if numbers[i] == target {
            return i;
        }
        i += 1;
    }
    numbers.len()
}

fn sum(values: &Vec<i64>) -> i64 {
    let mut total: i64 = 0;
    for i in 0..values.len() {
        total += values[i];
    }
    total
}

fn main() {
    let numbers: Vec<i64> = vec![10, 20, 30, 40];
    println!("{}", numbers.len());
    println!("{}", index_of(&numbers, 30));
    println!("{}", index_of(&numbers, 99));
    println!("{}", sum(&numbers));

    let mut index: usize = 1;
    index += 1;
    println!("{}", numbers[index]);
    println!("{}", numbers.len() > 2usize);
}