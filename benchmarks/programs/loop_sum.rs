fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
}

fn main() {
    // Optional argument: n; the default is the medium scale.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n = arg_i64(&args, 0, 8_000_000);
    let mut sum: i64 = 0;
    for i in 1..=n {
        sum += i;
    }
    println!("checksum={}", sum);
}
