"""string_split: build a comma-separated string of 50,000 decimal numbers
(about 194,500 bytes), split it, parse every field and sum it."""

TOKENS = 50_000


def main():
    parts = []
    for i in range(TOKENS):
        parts.append(str(i % 1000) + ",")
    text = "".join(parts)

    count = 0
    total = 0
    for part in text.split(","):
        if part:
            total += int(part)
            count += 1
    print(f"checksum={len(text)},{count},{total}")


if __name__ == "__main__":
    main()
