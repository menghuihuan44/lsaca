def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


def main():
    n = 30
    reps = 10
    checksum = 0
    for _ in range(reps):
        checksum += fib(n)
    print("checksum=%d" % checksum)


if __name__ == "__main__":
    main()
