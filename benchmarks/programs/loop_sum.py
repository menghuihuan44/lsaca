def main():
    n = 8000000
    total = 0
    for i in range(1, n + 1):
        total += i
    print("checksum=%d" % total)


if __name__ == "__main__":
    main()
