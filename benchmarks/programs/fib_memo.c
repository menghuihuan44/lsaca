/* fib_memo: recursive fibonacci with a memo table - another implementation of
 * the fib specification (same checksum). Optional arguments: n [repetitions];
 * the defaults are the medium scale. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define MEMO_CAPACITY 100

static int64_t memo[MEMO_CAPACITY];

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

static void memo_init(void) {
    for (int64_t i = 0; i < MEMO_CAPACITY; i++) {
        memo[i] = -1;
    }
}

static int64_t fib_memo(int64_t n) {
    if (n < 2) {
        return n;
    }
    if (memo[n] >= 0) {
        return memo[n];
    }
    const int64_t value = fib_memo(n - 1) + fib_memo(n - 2);
    memo[n] = value;
    return value;
}

int main(int argc, char **argv) {
    const int64_t n = arg_i64(argc, argv, 0, 30);
    const int64_t reps = arg_i64(argc, argv, 1, 10);
    memo_init();
    int64_t checksum = 0;
    for (int64_t r = 0; r < reps; r++) {
        checksum += fib_memo(n);
    }
    printf("checksum=%" PRId64 "\n", checksum);
    return 0;
}