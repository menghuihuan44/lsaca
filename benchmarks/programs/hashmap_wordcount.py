# hashmap_wordcount: count 200,000 deterministic words (4,096 distinct) in a
# dict keyed by the word text, then print a checksum over the frequencies, the
# number of distinct keys and the highest frequency. The checksum (sum of
# squared counts) is independent of dict iteration order.
def main():
    tokens = 200000
    distinct = 4096
    counts = {}
    for i in range(tokens):
        word = "w%d" % (i % distinct)
        counts[word] = counts.get(word, 0) + 1
    checksum = 0
    max_count = 0
    for count in counts.values():
        checksum += count * count
        if count > max_count:
            max_count = count
    print("checksum=%d,%d,%d" % (checksum, len(counts), max_count))


if __name__ == "__main__":
    main()
