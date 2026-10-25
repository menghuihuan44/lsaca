// sieve: count primes below 2,000,000 with a flat Float64Array of i64-sized
// flags (0 = still prime). The flag representation is the same in every
// language.
const LIMIT = 2000000;

function main() {
  const flags = new Float64Array(LIMIT + 1);
  let primes = 0;
  for (let i = 2; i <= LIMIT; i++) {
    if (flags[i] === 0) {
      primes++;
      for (let j = i * i; j <= LIMIT; j += i) {
        flags[j] = 1;
      }
    }
  }
  console.log(`checksum=${primes}`);
}

main();
