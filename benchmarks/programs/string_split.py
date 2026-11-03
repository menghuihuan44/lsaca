"""string_split: build a comma-separated string of decimal numbers, split it,
parse every field and sum it.

Optional argument: token count (default 50,000, the medium scale).
"""

import sys


def arg_int(args, index, fallback):
    if index < len(args):
        return int(args[index])
    return fallback


def main():
    tokens = arg_int(sys.argv[1:], 0, 50_000)
    parts = []
    for i in range(tokens):
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
