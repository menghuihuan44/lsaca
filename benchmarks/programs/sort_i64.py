"""sort_i64: pseudo-random i64 values from a Park-Miller LCG, sorted in place
with an iterative quicksort (explicit stack, middle pivot, Hoare partition).
Prints first,last,sum so the checksum covers ordering and the whole array.

Optional argument: value count (default 200,000, the medium scale).
"""

import sys

SEED = 12345


def lcg_next(state):
    return (16807 * state) % 2147483647


def arg_int(args, index, fallback):
    if index < len(args):
        return int(args[index])
    return fallback


def main():
    n = arg_int(sys.argv[1:], 0, 200_000)
    values = []
    state = SEED
    for _ in range(n):
        state = lcg_next(state)
        values.append(state)

    stack = [0, n - 1]
    while stack:
        hi = stack.pop()
        lo = stack.pop()
        if lo >= hi:
            continue
        mid = lo + (hi - lo) // 2
        pivot = values[mid]
        left = lo
        right = hi
        while left <= right:
            while values[left] < pivot:
                left += 1
            while values[right] > pivot:
                right -= 1
            if left <= right:
                values[left], values[right] = values[right], values[left]
                left += 1
                right -= 1
        if right > lo:
            stack.extend((lo, right))
        if left < hi:
            stack.extend((left, hi))

    print(f"checksum={values[0]},{values[-1]},{sum(values)}")


if __name__ == "__main__":
    main()
