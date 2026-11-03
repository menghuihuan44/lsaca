/* string_split: build a comma-separated string of decimal numbers, split it by
 * hand, parse each field and sum it.
 * Optional argument: token count (default 50,000, the medium scale). */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

int main(int argc, char **argv) {
    const int64_t tokens = arg_i64(argc, argv, 0, 50000);
    size_t capacity = (size_t)tokens * 5 + 1;
    char *text = (char *)malloc(capacity);
    if (text == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }

    size_t pos = 0;
    for (int64_t i = 0; i < tokens; i++) {
        int written = snprintf(text + pos, capacity - pos, "%" PRId64 ",", i % 1000);
        if (written < 0) {
            fprintf(stderr, "formatting failed\n");
            return 1;
        }
        pos += (size_t)written;
    }
    text[pos] = '\0';

    size_t len = strlen(text);
    int64_t count = 0;
    int64_t sum = 0;
    size_t start = 0;
    for (size_t i = 0; i <= pos; i++) {
        if (i == pos || text[i] == ',') {
            if (i > start) {
                char *end = NULL;
                int64_t value = strtoll(text + start, &end, 10);
                sum += value;
                count++;
            }
            start = i + 1;
        }
    }
    printf("checksum=%" PRIu64 ",%" PRId64 ",%" PRId64 "\n",
           (uint64_t)len, count, sum);
    free(text);
    return 0;
}
