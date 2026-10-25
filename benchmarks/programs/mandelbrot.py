"""mandelbrot: 600x400 grid, at most 100 iterations per pixel, classic
escape-time loop in f64; the checksum is the sum of all iteration counts."""

WIDTH = 600
HEIGHT = 400
MAX_ITER = 100


def mandel_iter(cr, ci):
    zx = 0.0
    zy = 0.0
    iter_count = 0
    while iter_count < MAX_ITER:
        zx2 = zx * zx
        zy2 = zy * zy
        if zx2 + zy2 > 4.0:
            break
        zy = 2.0 * zx * zy + ci
        zx = zx2 - zy2 + cr
        iter_count += 1
    return iter_count


def main():
    total = 0
    for py in range(HEIGHT):
        ci = py / HEIGHT * 2.0 - 1.0
        for px in range(WIDTH):
            cr = px / WIDTH * 3.5 - 2.5
            total += mandel_iter(cr, ci)
    print(f"checksum={total}")


if __name__ == "__main__":
    main()
