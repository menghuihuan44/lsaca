fn main() {
    let n: i64 = 8000000;
    let mut i: i64 = 1;
    let mut sum: i64 = 0;
    while i <= n {
        sum += i;
        i += 1;
    }
    println!("checksum={}", sum);
}
