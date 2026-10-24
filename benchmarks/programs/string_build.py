def main():
    tokens = 1000000
    parts = []
    token_sum = 0
    for i in range(tokens):
        v = i % 1000
        token_sum += v
        parts.append(str(v))
    s = ",".join(parts) + ","
    print("checksum=%d,%d" % (len(s), token_sum))


if __name__ == "__main__":
    main()
