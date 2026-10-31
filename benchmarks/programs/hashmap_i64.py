# hashmap_i64: 1,000,000 integer-key inserts, 1,000,000 present lookups,
# 1,000,000 absent lookups and 100,000 deletions in a dict; the folded
# checksum, the final size and the number of successful lookups are printed.
MOD = 1000000007


def mix(fold, value):
    return (fold * 131 + value) % MOD


def key(i):
    return (i * 2654435761) % 2147483648


def main():
    n = 1000000
    m = {}
    i = 0
    while i < n:
        k = key(i)
        m[k] = (k % 10007) + 1
        i += 1
    fold = 0
    hits = 0
    i = 0
    while i < n:
        value = m.get(key(i))
        if value is not None:
            hits += 1
            fold = mix(fold, value)
        i += 1
    i = 0
    while i < n:
        k = 2147483648 + i
        value = m.get(k)
        if value is None:
            fold = mix(fold, k % 97)
        else:
            fold = mix(fold, value)
        i += 1
    d = 0
    while d < 100000:
        removed = m.pop(key(d * 10), None)
        if removed is not None:
            fold = mix(fold, removed)
        else:
            fold = mix(fold, 1)
        d += 1
    print("checksum=%d,%d,%d" % (fold, len(m), hits))


if __name__ == "__main__":
    main()
