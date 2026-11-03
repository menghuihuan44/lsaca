/* sort_i64: pseudo-random i64 values from a Park-Miller LCG, sorted in place
 * with an iterative quicksort (explicit stack, middle pivot, Hoare partition).
 * Prints first,last,sum so the checksum covers ordering and the whole array.
 * Optional argument: value count (default 200,000, the medium scale). */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

#define SEED 12345

static int64_t lcg_next(int64_t state) {
    return (16807 * state) % 2147483647;
}

int main(int argc, char **argv) {
    const int64_t n = arg_i64(argc, argv, 0, 200000);
    int64_t *values = (int64_t *)malloc(sizeof(int64_t) * (size_t)n);
    int64_t *stack = (int64_t *)malloc(sizeof(int64_t) * 2 * (size_t)(n + 1));
    if (values == NULL || stack == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }

    int64_t state = SEED;
    for (int64_t i = 0; i < n; i++) {
        state = lcg_next(state);
        values[i] = state;
    }

    int64_t sp = 0;
    stack[sp++] = 0;
    stack[sp++] = n - 1;
    while (sp > 0) {
        int64_t hi = stack[--sp];
        int64_t lo = stack[--sp];
        if (lo >= hi) {
            continue;
        }
        int64_t mid = lo + (hi - lo) / 2;
        int64_t pivot = values[mid];
        int64_t left = lo;
        int64_t right = hi;
        while (left <= right) {
            while (values[left] < pivot) {
                left++;
            }
            while (values[right] > pivot) {
                right--;
            }
            if (left <= right) {
                int64_t tmp = values[left];
                values[left] = values[right];
                values[right] = tmp;
                left++;
                right--;
            }
        }
        if (right > lo) {
            stack[sp++] = lo;
            stack[sp++] = right;
        }
        if (left < hi) {
            stack[sp++] = left;
            stack[sp++] = hi;
        }
    }

    int64_t sum = 0;
    for (int64_t i = 0; i < n; i++) {
        sum += values[i];
    }
    printf("checksum=%" PRId64 ",%" PRId64 ",%" PRId64 "\n", values[0], values[n - 1], sum);
    free(values);
    free(stack);
    return 0;
}
