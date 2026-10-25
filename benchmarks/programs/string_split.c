/* string_split: build a comma-separated string of 50,000 decimal numbers
 * (about 194,500 bytes), split it by hand, parse each field and sum it. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TOKENS 50000

int main(void) {
    size_t capacity = (size_t)TOKENS * 5 + 1;
    char *text = (char *)malloc(capacity);
    if (text == NULL) {
        fprintf(stderr, "allocation failed\n");
        return 1;
    }

    size_t pos = 0;
    for (int64_t i = 0; i < TOKENS; i++) {
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
