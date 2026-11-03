"""loop_sum: integer sum 1..=n.

Optional argument: n; the default is the medium scale.
"""

import sys


def arg_int(args, index, fallback):
    if index < len(args):
        return int(args[index])
    return fallback


def main():
    n = arg_int(sys.argv[1:], 0, 8000000)
    total = 0
    for i in range(1, n + 1):
        total += i
    print("checksum=%d" % total)


if __name__ == "__main__":
    main()
