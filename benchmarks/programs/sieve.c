/* sieve: count primes below 2,000,000 with a flat i64 flag array (0 = still
 * prime). The flag representation is the same in every language. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define LIMIT 2000000

int main(void) {
    size_t count = (size_t)LIMIT + 1;
    int64_t *flags = (int64_t *)calloc(count, sizeof(int64_t));
    if (flags == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }

    int64_t primes = 0;
    for (int64_t i = 2; i <= LIMIT; i++) {
        if (flags[i] == 0) {
            primes++;
            for (int64_t j = i * i; j <= LIMIT; j += i) {
                flags[j] = 1;
            }
        }
    }
    printf("checksum=%" PRId64 "\n", primes);
    free(flags);
    return 0;
}
