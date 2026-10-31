# hashmap_string: 200,000 distinct generated String keys inserted into a dict,
# then 200,000 present lookups, 200,000 absent lookups and 20,000 deletions;
# the folded checksum, the final size and the number of successful lookups
# are printed.
MOD = 1000000007


def mix(fold, value):
    return (fold * 131 + value) % MOD


def key(i):
    return "k%d-%d" % (i % 1000, i)


def main():
    tokens = 200000
    m = {}
    i = 0
    while i < tokens:
        m[key(i)] = (i % 10007) + 1
        i += 1
    fold = 0
    hits = 0
    i = 0
    while i < tokens:
        value = m.get(key(i))
        if value is not None:
            hits += 1
            fold = mix(fold, value)
        i += 1
    i = 0
    while i < tokens:
        absent = key(tokens + i)
        value = m.get(absent)
        if value is None:
            for byte in absent.encode("utf-8"):
                fold = mix(fold, byte)
        else:
            fold = mix(fold, value)
        i += 1
    d = 0
    while d < 20000:
        removed = m.pop(key(d * 10), None)
        if removed is not None:
            fold = mix(fold, removed)
        else:
            fold = mix(fold, 1)
        d += 1
    print("checksum=%d,%d,%d" % (fold, len(m), hits))


if __name__ == "__main__":
    main()
