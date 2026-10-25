// sieve: count primes below 2,000,000 with a flat Vec<i64> flag array
// (0 = still prime). The flag representation is the same in every language.
fn main() {
    let limit: i64 = 2000000;
    let mut flags = vec![0; limit + 1];
    let mut primes = 0;
    let mut i = 2;
    while i <= limit {
        if flags[i] == 0 {
            primes += 1;
            let mut j = i * i;
            while j <= limit {
                flags[j] = 1;
                j += i;
            }
        }
        i += 1;
    }
    println!("checksum={}", primes);
}
