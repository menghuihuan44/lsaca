// mandelbrot: 600x400 grid, at most 100 iterations per pixel, classic
// escape-time loop in f64; the checksum is the sum of all iteration counts.
fn mandel_iter(cr: f64, ci: f64) -> i64 {
    let mut zx: f64 = 0.0;
    let mut zy: f64 = 0.0;
    let mut iter = 0;
    while iter < 100 {
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
    let width: i64 = 600;
    let height: i64 = 400;
    let mut total = 0;
    let mut py = 0;
    while py < height {
        let ci = (py as f64) / (height as f64) * 2.0 - 1.0;
        let mut px = 0;
        while px < width {
            let cr = (px as f64) / (width as f64) * 3.5 - 2.5;
            total += mandel_iter(cr, ci);
            px += 1;
        }
        py += 1;
    }
    println!("checksum={}", total);
}
