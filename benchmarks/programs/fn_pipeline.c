// fn_pipeline: a rule record carries a label string and a function value in the
// same struct. run() borrows the rule and calls its function value 1,000 times;
// the checksum sweeps three rules plus a copy of the first one, and reports the
// total label byte length.
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct {
    const char *label;
    int64_t (*apply)(int64_t);
} Rule;

static int64_t add3(int64_t x) {
    return x + 3;
}

static int64_t modstep(int64_t x) {
    return (x * 3 + 1) % 1000003;
}

static int64_t collatz(int64_t x) {
    if (x % 2 == 0) {
        return x / 2;
    }
    return x * 3 + 1;
}

static int64_t run(const Rule *rule, int64_t seed, int64_t times) {
    int64_t value = seed;
    for (int64_t i = 0; i < times; i++) {
        value = rule->apply(value);
    }
    return value;
}

int main(void) {
    Rule rules[3] = {
        {"add3", add3},
        {"modstep", modstep},
        {"collatz", collatz},
    };
    int64_t checksum = 0;
    int64_t label_bytes = 0;
    for (int64_t i = 0; i < 3; i++) {
        checksum += run(&rules[i], 7, 1000);
        label_bytes += (int64_t)strlen(rules[i].label);
    }
    Rule copy = rules[0];
    checksum += run(&copy, 11, 500);
    label_bytes += (int64_t)strlen(copy.label);
    printf("checksum=%lld,%lld\n", (long long)checksum, (long long)label_bytes);
    return 0;
}
