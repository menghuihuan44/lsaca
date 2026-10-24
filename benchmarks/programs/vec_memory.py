def main():
    n = 1000000
    points = []
    for i in range(n):
        points.append((i, 2 * i))
    checksum = 0
    for x, y in points:
        checksum += x + y
    print("checksum=%d" % checksum)


if __name__ == "__main__":
    main()
