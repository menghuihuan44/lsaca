"""sieve: count primes below a limit with a flat list of i64-sized int flags
(0 = still prime). The flag representation is the same in every language.

Optional argument: limit (default 2,000,000, the medium scale).
"""

import sys


def arg_int(args, index, fallback):
    if index < len(args):
        return int(args[index])
    return fallback


def main():
    limit = arg_int(sys.argv[1:], 0, 2_000_000)
    flags = [0] * (limit + 1)
    primes = 0
    for i in range(2, limit + 1):
        if flags[i] == 0:
            primes += 1
            j = i * i
            while j <= limit:
                flags[j] = 1
                j += i
    print(f"checksum={primes}")


if __name__ == "__main__":
    main()
