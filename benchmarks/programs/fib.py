"""fib: recursive fibonacci, called several times.

Optional arguments: n [repetitions]; the defaults are the medium scale.
"""

import sys


def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


def arg_int(args, index, fallback):
    if index < len(args):
        return int(args[index])
    return fallback


def main():
    args = sys.argv[1:]
    n = arg_int(args, 0, 30)
    reps = arg_int(args, 1, 10)
    checksum = 0
    for _ in range(reps):
        checksum += fib(n)
    print("checksum=%d" % checksum)


if __name__ == "__main__":
    main()
