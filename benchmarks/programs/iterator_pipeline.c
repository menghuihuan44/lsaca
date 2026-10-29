// iterator_pipeline: build 1,000,000 i64 values, then run "keep even values,
// square them, add them up" as a plain eager loop (the same arithmetic as the
// lazy iterator chain in the lsaca and Rust versions) and print the folded
// sum. The checksum is order-independent.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int64_t *make_numbers(int64_t n) {
    int64_t *numbers = (int64_t *)malloc(sizeof(int64_t) * (size_t)n);
    if (numbers == NULL) {
        fprintf(stderr, "out of memory\n");
        return NULL;
    }
    for (int64_t i = 0; i < n; i++) {
        numbers[i] = i;
    }
    return numbers;
}

int main(void) {
    const int64_t n = 1000000;
    int64_t *numbers = make_numbers(n);
    if (numbers == NULL) {
        return 1;
    }
    int64_t total = 0;
    for (int64_t i = 0; i < n; i++) {
        int64_t value = numbers[i];
        if (value % 2 == 0) {
            total += value * value;
        }
    }
    free(numbers);
    printf("checksum=%lld\n", (long long)total);
    return 0;
}
