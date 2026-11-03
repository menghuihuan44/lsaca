// sieve: count primes below a limit with a flat i64 flag array (0 = still
// prime). The flag representation is the same in every language.
// Optional argument: limit (default 2,000,000, the medium scale).
fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let limit = arg_i64(&args, 0, 2_000_000) as usize;
    let mut flags = vec![0i64; limit + 1];
    let mut primes = 0i64;
    for i in 2..=limit {
        if flags[i] == 0 {
            primes += 1;
            let mut j = i * i;
            while j <= limit {
                flags[j] = 1;
                j += i;
            }
        }
    }
    println!("checksum={}", primes);
}
