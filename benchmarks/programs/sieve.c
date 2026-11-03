/* sieve: count primes below a limit with a flat i64 flag array (0 = still
 * prime). The flag representation is the same in every language.
 * Optional argument: limit (default 2,000,000, the medium scale). */
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

int main(int argc, char **argv) {
    const int64_t limit = arg_i64(argc, argv, 0, 2000000);
    size_t count = (size_t)limit + 1;
    int64_t *flags = (int64_t *)calloc(count, sizeof(int64_t));
    if (flags == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }

    int64_t primes = 0;
    for (int64_t i = 2; i <= limit; i++) {
        if (flags[i] == 0) {
            primes++;
            for (int64_t j = i * i; j <= limit; j += i) {
                flags[j] = 1;
            }
        }
    }
    printf("checksum=%" PRId64 "\n", primes);
    free(flags);
    return 0;
}
