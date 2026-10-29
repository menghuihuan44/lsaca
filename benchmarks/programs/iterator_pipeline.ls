// iterator_pipeline: build 1,000,000 i64 values, then run the lazy pipeline
// iter().filter(even).map(x * x).fold(0, add) and print the folded sum. The
// checksum is order-independent.
fn make_numbers(n: i64) -> Vec<i64> {
    let mut values: Vec<i64> = Vec::with_capacity(n);
    let mut value = 0;
    while value < n {
        values.push(value);
        value += 1;
    }
    values
}

fn main() {
    let values = make_numbers(1000000);
    let total: i64 = values.iter().filter(|x| *x % 2 == 0).map(|x| x * x).fold(0, |acc, x| acc + x);
    println!("checksum={}", total);
}
