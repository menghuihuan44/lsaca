// sort_i64: pseudo-random i64 values from a Park-Miller LCG, sorted in place
// with an iterative quicksort (explicit stack, middle pivot, Hoare partition).
// Prints first,last,sum so the checksum covers ordering and the whole array.
// Optional argument: value count (default 200,000, the medium scale).
'use strict';

const SEED = 12345;

function lcgNext(state) {
  return (16807 * state) % 2147483647;
}

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const n = argInt(process.argv.slice(2), 0, 200000);
const values = new Array(n);
let state = SEED;
for (let i = 0; i < n; i++) {
  state = lcgNext(state);
  values[i] = state;
}

const stack = [0, n - 1];
while (stack.length > 0) {
  const hi = stack.pop();
  const lo = stack.pop();
  if (lo >= hi) {
    continue;
  }
  const mid = lo + Math.floor((hi - lo) / 2);
  const pivot = values[mid];
  let left = lo;
  let right = hi;
  while (left <= right) {
    while (values[left] < pivot) {
      left += 1;
    }
    while (values[right] > pivot) {
      right -= 1;
    }
    if (left <= right) {
      const tmp = values[left];
      values[left] = values[right];
      values[right] = tmp;
      left += 1;
      right -= 1;
    }
  }
  if (right > lo) {
    stack.push(lo);
    stack.push(right);
  }
  if (left < hi) {
    stack.push(left);
    stack.push(hi);
  }
}

let sum = 0;
for (let i = 0; i < n; i++) {
  sum += values[i];
}
console.log('checksum=' + values[0] + ',' + values[n - 1] + ',' + sum);