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
    // Optional argument: token count; the default is the medium scale.
    const int64_t tokens = arg_i64(argc, argv, 0, 1000000);
    char *buf = (char *)malloc((size_t)tokens * 4 + 1);
    if (buf == NULL) {
        return 1;
    }
    int64_t len = 0;
    int64_t token_sum = 0;
    for (int64_t i = 0; i < tokens; i++) {
        int64_t v = i % 1000;
        token_sum += v;
        if (v >= 100) {
            buf[len++] = (char)('0' + v / 100);
            buf[len++] = (char)('0' + (v / 10) % 10);
            buf[len++] = (char)('0' + v % 10);
        } else if (v >= 10) {
            buf[len++] = (char)('0' + v / 10);
            buf[len++] = (char)('0' + v % 10);
        } else {
            buf[len++] = (char)('0' + v);
        }
        buf[len++] = ',';
    }
    buf[len] = '\0';
    printf("checksum=%lld,%lld\n", (long long)len, (long long)token_sum);
    free(buf);
    return 0;
}
