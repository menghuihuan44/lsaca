#include <stdint.h>
#include <stdio.h>

int main(void) {
    const int64_t n = 8000000;
    int64_t sum = 0;
    for (int64_t i = 1; i <= n; i++) {
        sum += i;
    }
    printf("checksum=%lld\n", (long long)sum);
    return 0;
}
