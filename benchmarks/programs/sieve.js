// sieve: count primes below a limit with a flat array of i64-sized integer
// flags (0 = still prime). The flag representation is the same in every
// language.
// Optional argument: limit (default 2,000,000, the medium scale).
'use strict';

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const limit = argInt(process.argv.slice(2), 0, 2000000);
const flags = new Array(limit + 1).fill(0);
let primes = 0;
for (let i = 2; i <= limit; i++) {
  if (flags[i] === 0) {
    primes += 1;
    let j = i * i;
    while (j <= limit) {
      flags[j] = 1;
      j += i;
    }
  }
}
console.log('checksum=' + primes);