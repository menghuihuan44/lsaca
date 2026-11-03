fn main() {
    // Optional argument: n; the default is the medium scale.
    let all = args();
    let n: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 8000000 };
    let mut i: i64 = 1;
    let mut sum: i64 = 0;
    while i <= n {
        sum += i;
        i += 1;
    }
    println!("checksum={}", sum);
}
