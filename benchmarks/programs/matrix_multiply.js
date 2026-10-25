// matrix_multiply: two deterministic 160x160 i64 matrices in flat Float64Arrays
// (index i*n+j), naive i-j-k triple loop into a third flat array, then the sum
// of every result element.
const N = 160;

function valueA(i, j) {
  return (i * 31 + j * 17) % 1000;
}

function valueB(i, j) {
  return (i * 13 + j * 7) % 1000;
}

function main() {
  const cells = N * N;
  const a = new Float64Array(cells);
  const b = new Float64Array(cells);
  const c = new Float64Array(cells);
  for (let i = 0; i < N; i++) {
    for (let j = 0; j < N; j++) {
      a[i * N + j] = valueA(i, j);
      b[i * N + j] = valueB(i, j);
    }
  }
  for (let i = 0; i < N; i++) {
    for (let j = 0; j < N; j++) {
      let sum = 0;
      for (let k = 0; k < N; k++) {
        sum += a[i * N + k] * b[k * N + j];
      }
      c[i * N + j] = sum;
    }
  }
  let total = 0;
  for (let idx = 0; idx < cells; idx++) total += c[idx];
  console.log(`checksum=${total}`);
}

main();
