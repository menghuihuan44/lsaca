fn main() {
    let n: i64 = 8_000_000;
    let mut sum: i64 = 0;
    for i in 1..=n {
        sum += i;
    }
    println!("checksum={}", sum);
}
