// mandelbrot: 600x400 grid, at most 100 iterations per pixel, classic
// escape-time loop in f64; the checksum is the sum of all iteration counts.
const WIDTH = 600;
const HEIGHT = 400;
const MAX_ITER = 100;

function mandelIter(cr, ci) {
  let zx = 0.0;
  let zy = 0.0;
  let iter = 0;
  while (iter < MAX_ITER) {
    const zx2 = zx * zx;
    const zy2 = zy * zy;
    if (zx2 + zy2 > 4.0) break;
    zy = 2.0 * zx * zy + ci;
    zx = zx2 - zy2 + cr;
    iter++;
  }
  return iter;
}

function main() {
  let total = 0;
  for (let py = 0; py < HEIGHT; py++) {
    const ci = (py / HEIGHT) * 2.0 - 1.0;
    for (let px = 0; px < WIDTH; px++) {
      const cr = (px / WIDTH) * 3.5 - 2.5;
      total += mandelIter(cr, ci);
    }
  }
  console.log(`checksum=${total}`);
}

main();
