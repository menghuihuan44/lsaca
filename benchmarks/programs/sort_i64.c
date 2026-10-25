/* sort_i64: 200,000 pseudo-random i64 values from a Park-Miller LCG,
 * sorted in place with an iterative quicksort (explicit stack, middle pivot,
 * Hoare partition). Prints first,last,sum so the checksum covers ordering and
 * the whole array. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define N 200000
#define SEED 12345

static int64_t lcg_next(int64_t state) {
    return (16807 * state) % 2147483647;
}

int main(void) {
    int64_t *values = (int64_t *)malloc(sizeof(int64_t) * (size_t)N);
    int64_t *stack = (int64_t *)malloc(sizeof(int64_t) * 2 * (size_t)(N + 1));
    if (values == NULL || stack == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }

    int64_t state = SEED;
    for (int64_t i = 0; i < N; i++) {
        state = lcg_next(state);
        values[i] = state;
    }

    int64_t sp = 0;
    stack[sp++] = 0;
    stack[sp++] = N - 1;
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
    for (int64_t i = 0; i < N; i++) {
        sum += values[i];
    }
    printf("checksum=%" PRId64 ",%" PRId64 ",%" PRId64 "\n", values[0], values[N - 1], sum);
    free(values);
    free(stack);
    return 0;
}
