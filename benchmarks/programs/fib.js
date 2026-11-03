// fib: recursive fibonacci, called several times.
// Optional arguments: n [repetitions]; the defaults are the medium scale.
'use strict';

function fib(n) {
  if (n < 2) {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const argv = process.argv.slice(2);
const n = argInt(argv, 0, 30);
const reps = argInt(argv, 1, 10);
let checksum = 0;
for (let i = 0; i < reps; i++) {
  checksum += fib(n);
}
console.log('checksum=' + checksum);