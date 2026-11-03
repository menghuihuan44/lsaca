"""string_build: append numeric tokens to a string.

Optional argument: token count; the default is the medium scale.
"""

import sys


def arg_int(args, index, fallback):
    if index < len(args):
        return int(args[index])
    return fallback


def main():
    tokens = arg_int(sys.argv[1:], 0, 1000000)
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
