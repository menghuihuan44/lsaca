/* matrix_multiply: two deterministic 160x160 i64 matrices in flat arrays
 * (index i*n+j), naive i-j-k triple loop into a third flat array, then the sum
 * of every result element. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define N 160

static int64_t value_a(int64_t i, int64_t j) {
    return (i * 31 + j * 17) % 1000;
}

static int64_t value_b(int64_t i, int64_t j) {
    return (i * 13 + j * 7) % 1000;
}

int main(void) {
    size_t cells = (size_t)N * (size_t)N;
    int64_t *a = (int64_t *)malloc(sizeof(int64_t) * cells);
    int64_t *b = (int64_t *)malloc(sizeof(int64_t) * cells);
    int64_t *c = (int64_t *)malloc(sizeof(int64_t) * cells);
    if (a == NULL || b == NULL || c == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }

    for (int64_t i = 0; i < N; i++) {
        for (int64_t j = 0; j < N; j++) {
            a[i * N + j] = value_a(i, j);
            b[i * N + j] = value_b(i, j);
        }
    }

    for (int64_t i = 0; i < N; i++) {
        for (int64_t j = 0; j < N; j++) {
            int64_t sum = 0;
            for (int64_t k = 0; k < N; k++) {
                sum += a[i * N + k] * b[k * N + j];
            }
            c[i * N + j] = sum;
        }
    }

    int64_t total = 0;
    for (size_t idx = 0; idx < cells; idx++) {
        total += c[idx];
    }
    printf("checksum=%" PRId64 "\n", total);
    free(a);
    free(b);
    free(c);
    return 0;
}
