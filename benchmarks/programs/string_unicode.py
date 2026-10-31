# string_unicode: exercise the UTF-8 string API over a deterministic
# mixed-ASCII/Unicode corpus: push 120,000 scalars one at a time, fold the
# scalars from iteration and char_at, apply full-Unicode to_uppercase, slice
# with substring at character boundaries, extend, pop and fold again.
# The checksum is <char_count>,<byte_len>,<folded scalar checksum>.
MOD = 1000000007


def mix(fold, value):
    return (fold * 131 + value) % MOD


def main():
    alphabet = [ord(c) for c in "a\u00e9\u00f1\u00fc\u00f6\u00df\ufb01\u00e0\u00e7\u03b1\u03b2\u03b3\u03b4\u0430\u0431\u0432\u4f60\u597d\u4e16\u754c\U0001F600kqw"]
    n = 120000
    k = len(alphabet)
    base = []
    i = 0
    while i < n:
        base.append(alphabet[i % k])
        i += 1
    fold = 0
    for cp in base:
        fold = mix(fold, cp)
    probes = 0
    while probes < 1000:
        fold = mix(fold, base[probes * 7])
        probes += 1
    upper = []
    for cp in base:
        upper.extend(ord(c) for c in chr(cp).upper())
    for cp in upper:
        fold = mix(fold, cp)
    offsets = []
    off = 0
    for cp in base:
        offsets.append(off)
        off += len(chr(cp).encode("utf-8"))
    offsets.append(off)
    mid = off // 2
    while mid not in offsets:
        mid -= 1
    end = mid + 5000
    while end not in offsets:
        end -= 1
    data = "".join(chr(cp) for cp in base).encode("utf-8")
    built = data[mid:end].decode("utf-8")
    built += "!"
    built += "".join(chr(cp) for cp in upper[:1000])
    popped = built[-1]
    fold = mix(fold, ord(popped))
    built = built[:-1] + "done"
    for ch in built:
        fold = mix(fold, ord(ch))
    print("checksum=%d,%d,%d" % (len(built), len(built.encode("utf-8")), fold))


if __name__ == "__main__":
    main()
