#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

static int64_t fib(int64_t n) {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

int main(int argc, char **argv) {
    // Optional arguments: n [repetitions]; the defaults are the medium scale.
    const int64_t n = arg_i64(argc, argv, 0, 30);
    const int64_t reps = arg_i64(argc, argv, 1, 10);
    int64_t checksum = 0;
    for (int64_t i = 0; i < reps; i++) {
        checksum += fib(n);
    }
    printf("checksum=%lld\n", (long long)checksum);
    return 0;
}
