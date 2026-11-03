#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

int main(int argc, char **argv) {
    // Optional argument: n; the default is the medium scale.
    const int64_t n = arg_i64(argc, argv, 0, 8000000);
    int64_t sum = 0;
    for (int64_t i = 1; i <= n; i++) {
        sum += i;
    }
    printf("checksum=%lld\n", (long long)sum);
    return 0;
}
