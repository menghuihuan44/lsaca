// mandelbrot: 600x400 grid, at most 100 iterations per pixel, classic
// escape-time loop in f64; the checksum is the sum of all iteration counts.
const WIDTH: i64 = 600;
const HEIGHT: i64 = 400;
const MAX_ITER: i64 = 100;

fn mandel_iter(cr: f64, ci: f64) -> i64 {
    let mut zx = 0.0f64;
    let mut zy = 0.0f64;
    let mut iter = 0i64;
    while iter < MAX_ITER {
        let zx2 = zx * zx;
        let zy2 = zy * zy;
        if zx2 + zy2 > 4.0 {
            break;
        }
        zy = 2.0 * zx * zy + ci;
        zx = zx2 - zy2 + cr;
        iter += 1;
    }
    iter
}

fn main() {
    let mut total = 0i64;
    for py in 0..HEIGHT {
        let ci = py as f64 / HEIGHT as f64 * 2.0 - 1.0;
        for px in 0..WIDTH {
            let cr = px as f64 / WIDTH as f64 * 3.5 - 2.5;
            total += mandel_iter(cr, ci);
        }
    }
    println!("checksum={}", total);
}
