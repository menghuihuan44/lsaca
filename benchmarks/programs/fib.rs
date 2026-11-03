fn fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
}

fn main() {
    // Optional arguments: n [repetitions]; the defaults are the medium scale.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n = arg_i64(&args, 0, 30);
    let reps = arg_i64(&args, 1, 10);
    let mut checksum: i64 = 0;
    for _ in 0..reps {
        checksum += fib(n);
    }
    println!("checksum={}", checksum);
}
