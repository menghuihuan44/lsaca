// sieve: count primes below 2,000,000 with a flat i64 flag array (0 = still
// prime). The flag representation is the same in every language.
const LIMIT: usize = 2_000_000;

fn main() {
    let mut flags = vec![0i64; LIMIT + 1];
    let mut primes = 0i64;
    for i in 2..=LIMIT {
        if flags[i] == 0 {
            primes += 1;
            let mut j = i * i;
            while j <= LIMIT {
                flags[j] = 1;
                j += i;
            }
        }
    }
    println!("checksum={}", primes);
}
