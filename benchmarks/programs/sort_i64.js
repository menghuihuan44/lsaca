// sort_i64: 200,000 pseudo-random i64 values from a Park-Miller LCG, sorted in
// place with an iterative quicksort (explicit stack, middle pivot, Hoare
// partition). Prints first,last,sum so the checksum covers ordering and the
// whole array.
const N = 200000;
const SEED = 12345;

function lcgNext(state) {
  return (16807 * state) % 2147483647;
}

function main() {
  const values = new Array(N);
  let state = SEED;
  for (let i = 0; i < N; i++) {
    state = lcgNext(state);
    values[i] = state;
  }

  const stack = [0, N - 1];
  while (stack.length > 0) {
    const hi = stack.pop();
    const lo = stack.pop();
    if (lo >= hi) continue;
    const mid = lo + ((hi - lo) >> 1);
    const pivot = values[mid];
    let left = lo;
    let right = hi;
    while (left <= right) {
      while (values[left] < pivot) left++;
      while (values[right] > pivot) right--;
      if (left <= right) {
        const tmp = values[left];
        values[left] = values[right];
        values[right] = tmp;
        left++;
        right--;
      }
    }
    if (right > lo) stack.push(lo, right);
    if (left < hi) stack.push(left, hi);
  }

  let sum = 0;
  for (let i = 0; i < N; i++) sum += values[i];
  console.log(`checksum=${values[0]},${values[N - 1]},${sum}`);
}

main();
