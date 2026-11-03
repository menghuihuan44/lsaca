/* sort_insertion: insertion sort over the same LCG sequence as sort_i64 - the
 * alternative implementation for the sorting specification (same checksum
 * rule: first, last, sum). Insertion sort is O(n^2), so the sizes are small.
 * Optional argument: value count (default 1,000, the medium scale). */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define SEED 12345

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

static void insertion_sort(int64_t *values, int64_t n) {
    for (int64_t i = 1; i < n; i++) {
        const int64_t key = values[i];
        int64_t j = i - 1;
        while (j >= 0 && values[j] > key) {
            values[j + 1] = values[j];
            j -= 1;
        }
        values[j + 1] = key;
    }
}

int main(int argc, char **argv) {
    const int64_t n = arg_i64(argc, argv, 0, 1000);
    int64_t *values = (int64_t *)malloc((size_t)n * sizeof(int64_t));
    if (values == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }
    int64_t state = SEED;
    for (int64_t i = 0; i < n; i++) {
        state = (16807 * state) % 2147483647;
        values[i] = state;
    }
    insertion_sort(values, n);
    int64_t sum = 0;
    for (int64_t i = 0; i < n; i++) {
        sum += values[i];
    }
    printf("checksum=%" PRId64 ",%" PRId64 ",%" PRId64 "\n", values[0], values[n - 1], sum);
    free(values);
    return 0;
}