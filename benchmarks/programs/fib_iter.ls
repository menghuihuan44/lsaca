// fib_iter: iterative fibonacci - the alternative implementation twin of fib
// (recursive). Same specification and checksum; optional arguments:
// n [repetitions]; the defaults are the medium scale.
fn fib_iter(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let next = a + b;
        a = b;
        b = next;
        i = i + 1;
    }
    a
}

fn main() {
    let all = args();
    let n: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 30 };
    let reps: i64 = if all.len() > 1 { all[1].clone().parse_i64().unwrap() } else { 10 };
    let mut checksum: i64 = 0;
    let mut r: i64 = 0;
    while r < reps {
        checksum = checksum + fib_iter(n);
        r = r + 1;
    }
    println!("checksum={}", checksum);
}