fn fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn main() {
    let n: i64 = 30;
    let reps: i64 = 10;
    let mut checksum: i64 = 0;
    for _ in 0..reps {
        checksum += fib(n);
    }
    println!("checksum={}", checksum);
}
