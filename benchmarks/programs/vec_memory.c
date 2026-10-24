#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct {
    int64_t x;
    int64_t y;
} Point;

int main(void) {
    const int64_t n = 1000000;
    Point *points = (Point *)malloc(sizeof(Point) * (size_t)n);
    if (points == NULL) {
        return 1;
    }
    for (int64_t i = 0; i < n; i++) {
        points[i].x = i;
        points[i].y = 2 * i;
    }
    int64_t checksum = 0;
    for (int64_t i = 0; i < n; i++) {
        checksum += points[i].x + points[i].y;
    }
    printf("checksum=%lld\n", (long long)checksum);
    free(points);
    return 0;
}
