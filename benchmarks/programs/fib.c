#include <stdint.h>
#include <stdio.h>

static int64_t fib(int64_t n) {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

int main(void) {
    const int64_t n = 30;
    const int64_t reps = 10;
    int64_t checksum = 0;
    for (int64_t i = 0; i < reps; i++) {
        checksum += fib(n);
    }
    printf("checksum=%lld\n", (long long)checksum);
    return 0;
}
