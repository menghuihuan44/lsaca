/* mandelbrot: 600x400 grid, at most 100 iterations per pixel, classic
 * escape-time loop in f64; the checksum is the sum of all iteration counts. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

#define WIDTH 600
#define HEIGHT 400
#define MAX_ITER 100

static int64_t mandel_iter(double cr, double ci) {
    double zx = 0.0;
    double zy = 0.0;
    int64_t iter = 0;
    while (iter < MAX_ITER) {
        double zx2 = zx * zx;
        double zy2 = zy * zy;
        if (zx2 + zy2 > 4.0) {
            break;
        }
        zy = 2.0 * zx * zy + ci;
        zx = zx2 - zy2 + cr;
        iter++;
    }
    return iter;
}

int main(void) {
    int64_t total = 0;
    for (int64_t py = 0; py < HEIGHT; py++) {
        double ci = (double)py / (double)HEIGHT * 2.0 - 1.0;
        for (int64_t px = 0; px < WIDTH; px++) {
            double cr = (double)px / (double)WIDTH * 3.5 - 2.5;
            total += mandel_iter(cr, ci);
        }
    }
    printf("checksum=%" PRId64 "\n", total);
    return 0;
}
