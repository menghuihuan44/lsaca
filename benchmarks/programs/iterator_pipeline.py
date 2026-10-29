# iterator_pipeline: build 1,000,000 i64 values, then run "keep even values,
# square them, add them up" as a generator expression and print the folded sum.
# The checksum is order-independent.
def make_numbers(n):
    return list(range(n))


def main():
    values = make_numbers(1000000)
    total = sum(x * x for x in values if x % 2 == 0)
    print("checksum=%d" % total)


if __name__ == "__main__":
    main()
