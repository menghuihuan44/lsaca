"""matrix_multiply: two deterministic 160x160 i64 matrices in flat lists
(index i*n+j), naive i-j-k triple loop into a third flat list, then the sum of
every result element."""

N = 160


def value_a(i, j):
    return (i * 31 + j * 17) % 1000


def value_b(i, j):
    return (i * 13 + j * 7) % 1000


def main():
    cells = N * N
    a = [0] * cells
    b = [0] * cells
    c = [0] * cells
    for i in range(N):
        for j in range(N):
            a[i * N + j] = value_a(i, j)
            b[i * N + j] = value_b(i, j)
    for i in range(N):
        for j in range(N):
            total = 0
            for k in range(N):
                total += a[i * N + k] * b[k * N + j]
            c[i * N + j] = total
    print(f"checksum={sum(c)}")


if __name__ == "__main__":
    main()
