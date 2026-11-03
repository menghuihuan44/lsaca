/* fib_iter: iterative fibonacci - the alternative implementation twin of
 * fib (recursive). Same specification and checksum; optional arguments:
 * n [repetitions]; the defaults are the medium scale. */
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

static int64_t fib_iter(int64_t n) {
    int64_t a = 0;
    int64_t b = 1;
    int64_t i = 0;
    while (i < n) {
        const int64_t next = a + b;
        a = b;
        b = next;
        i += 1;
    }
    return a;
}

int main(int argc, char **argv) {
    const int64_t n = arg_i64(argc, argv, 0, 30);
    const int64_t reps = arg_i64(argc, argv, 1, 10);
    int64_t checksum = 0;
    for (int64_t r = 0; r < reps; r++) {
        checksum += fib_iter(n);
    }
    printf("checksum=%" PRId64 "\n", checksum);
    return 0;
}