// iterator_pipeline: build 1,000,000 i64 values, then run
// iter().filter(even).map(x * x).fold(0, add) as a lazy iterator chain and
// print the folded sum. The checksum is order-independent.
fn make_numbers(n: i64) -> Vec<i64> {
    (0..n).collect()
}

fn main() {
    let n: i64 = 1_000_000;
    let total: i64 = make_numbers(n)
        .iter()
        .filter(|x| *x % 2 == 0)
        .map(|x| x * x)
        .fold(0, |acc, x| acc + x);
    println!("checksum={}", total);
}
