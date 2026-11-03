/* array_wordcount: count the same deterministic word stream as
 * hashmap_wordcount in a flat array indexed by the word number (the word space
 * is fixed at 4096), then print the same frequency checksum. The alternative
 * trade-off: direct indexing instead of a hash map.
 * Optional argument: token count (default 200,000, the medium scale). */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define DISTINCT 4096

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

int main(int argc, char **argv) {
    const int64_t tokens = arg_i64(argc, argv, 0, 200000);
    int64_t counts[DISTINCT];
    for (int64_t i = 0; i < DISTINCT; i++) {
        counts[i] = 0;
    }
    for (int64_t token = 0; token < tokens; token++) {
        counts[token % DISTINCT] += 1;
    }
    int64_t checksum = 0;
    int64_t max_count = 0;
    for (int64_t k = 0; k < DISTINCT; k++) {
        const int64_t count = counts[k];
        checksum += count * count;
        if (count > max_count) {
            max_count = count;
        }
    }
    printf("checksum=%" PRId64 ",%" PRId64 ",%" PRId64 "\n", checksum, (int64_t)DISTINCT, max_count);
    return 0;
}