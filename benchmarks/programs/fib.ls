fn fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn main() {
    // Optional arguments: n [repetitions]; defaults are the medium scale.
    let all = args();
    let n: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 30 };
    let reps: i64 = if all.len() > 1 { all[1].clone().parse_i64().unwrap() } else { 10 };
    let mut checksum: i64 = 0;
    let mut i: i64 = 0;
    while i < reps {
        checksum += fib(n);
        i += 1;
    }
    println!("checksum={}", checksum);
}
