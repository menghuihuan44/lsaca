"""sieve: count primes below 2,000,000 with a flat list of i64-sized int flags
(0 = still prime). The flag representation is the same in every language."""

LIMIT = 2_000_000


def main():
    flags = [0] * (LIMIT + 1)
    primes = 0
    for i in range(2, LIMIT + 1):
        if flags[i] == 0:
            primes += 1
            j = i * i
            while j <= LIMIT:
                flags[j] = 1
                j += i
    print(f"checksum={primes}")


if __name__ == "__main__":
    main()
