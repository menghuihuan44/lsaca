/* benchmark driver for the lsaca C runtime (task-17 hot-path measurements)
 *
 * This program touches the PUBLIC runtime API only (runtime/lsaca_rt.h); it
 * does not depend on any internal header and is not part of the generated
 * project, the smoke test matrix, or CMake output.  It exists so the runtime
 * split/optimization work can be measured reproducibly:
 *
 *   gcc -std=c17 -O2 -DNDEBUG bench_runtime.c lsaca_rt.c -lm -o bench_runtime
 *   ./bench_runtime                 # timing sections, wall-clock ms, best of 3
 *   ./bench_runtime --peak-vec N    # peak RSS probe: Vec<i64> with N pushes
 *   ./bench_runtime --peak-vec-cap N# peak RSS probe: Vec with exact capacity N
 *   ./bench_runtime --peak-str N    # peak RSS probe: String grown by N chars
 *   ./bench_runtime --peak-tokens N # peak RSS probe: numeric token builder
 *
 * Every section returns a checksum that is printed, so the compiler cannot
 * remove the work; the checksums are deterministic.
 */

#include "lsaca_rt.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* ------------------------------------------------------------------ */
/* timing helpers                                                      */
/* ------------------------------------------------------------------ */

static double now_ms(void)
{
    struct timespec ts;
    if (timespec_get(&ts, TIME_UTC) == 0) {
        return 0.0;
    }
    return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1e6;
}

typedef unsigned long long (*SectionFn)(size_t n);

static void run_section(const char* name, size_t n, SectionFn fn)
{
    double best = -1.0;
    unsigned long long checksum = 0;
    int rep;
    for (rep = 0; rep < 3; rep++) {
        double start = now_ms();
        unsigned long long value = fn(n);
        double elapsed = now_ms() - start;
        checksum = value;
        if (best < 0.0 || elapsed < best) {
            best = elapsed;
        }
    }
    printf("%-20s n=%-9zu best=%10.3f ms  checksum=%llu\n", name, n, best,
           checksum);
}

/* ------------------------------------------------------------------ */
/* timing sections (all mimic the codegen call patterns)               */
/* ------------------------------------------------------------------ */

/* Vec::push codegen form: ls_vec_insert_bytes(..., len, &value, sizeof(T)). */
static unsigned long long sec_vec_push(size_t n)
{
    int64_t* data = NULL;
    size_t len = 0;
    size_t cap = 0;
    size_t i;
    unsigned long long sum = 0;

    for (i = 0; i < n; i++) {
        int64_t value = (int64_t)i;
        ls_vec_insert_bytes((void**)&data, &len, &cap, len, &value,
                            sizeof(value));
    }
    sum = (unsigned long long)len;
    if (len > 0) {
        sum += (unsigned long long)(data[0] + data[len - 1]);
    }
    ls_vec_free_bytes(data);
    return sum;
}

/* Direct grow + indexed store (the clone/with_capacity fill form). */
static unsigned long long sec_vec_grow(size_t n)
{
    int64_t* data = NULL;
    size_t len = 0;
    size_t cap = 0;
    size_t i;
    unsigned long long sum = 0;

    for (i = 0; i < n; i++) {
        ls_vec_grow((void**)&data, &cap, len, sizeof(int64_t));
        data[len] = (int64_t)i;
        len += 1;
    }
    sum = (unsigned long long)len;
    if (len > 0) {
        sum += (unsigned long long)(data[0] + data[len - 1]);
    }
    ls_vec_free_bytes(data);
    return sum;
}

/* Vec::with_capacity: exact buffer, direct fill, no growth. */
static unsigned long long sec_vec_exact(size_t n)
{
    int64_t* data = (int64_t*)ls_vec_alloc(n, sizeof(int64_t));
    size_t i;
    unsigned long long sum;

    if (n != 0 && data == NULL) {
        return 0;
    }
    for (i = 0; i < n; i++) {
        data[i] = (int64_t)i;
    }
    sum = (unsigned long long)n;
    if (n > 0) {
        sum += (unsigned long long)(data[0] + data[n - 1]);
    }
    ls_vec_free_bytes(data);
    return sum;
}

/* string_build workload: 1,000,000 tokens (i % 1000) each followed by ",";
 * final length must be 3,890,000 for n == 1,000,000. */
static unsigned long long sec_str_build(size_t n)
{
    LsString* acc = ls_format_begin(16);
    size_t i;
    unsigned long long len;

    for (i = 0; i < n; i++) {
        acc = ls_fmt_u64(acc, (uint64_t)(i % 1000));
        acc = ls_format_push_cstr(acc, ",");
    }
    len = (unsigned long long)ls_str_len(acc);
    ls_release(acc);
    return len;
}

/* Plain integer formatting throughput into one accumulator. */
static unsigned long long sec_fmt_u64(size_t n)
{
    LsString* acc = ls_format_begin(16);
    size_t i;
    unsigned long long len;

    for (i = 0; i < n; i++) {
        acc = ls_fmt_u64(acc, (uint64_t)(i % 1000));
    }
    len = (unsigned long long)ls_str_len(acc);
    ls_release(acc);
    return len;
}

static unsigned long long sec_str_push_char(size_t n)
{
    LsString* s = ls_str_new();
    size_t i;
    unsigned long long len;

    for (i = 0; i < n; i++) {
        s = ls_str_push_char(s, 'x');
    }
    len = (unsigned long long)ls_str_len(s);
    ls_release(s);
    return len;
}

static unsigned long long sec_str_push_cstr(size_t n)
{
    LsString* s = ls_str_new();
    size_t i;
    unsigned long long len;

    for (i = 0; i < n; i++) {
        s = ls_str_push_cstr(s, "abcdefghij");
    }
    len = (unsigned long long)ls_str_len(s);
    ls_release(s);
    return len;
}

/* Refcount allocator round trip: the payload is touched so the page is real. */
static unsigned long long sec_alloc_release(size_t n)
{
    size_t i;
    unsigned long long sum = 0;

    for (i = 0; i < n; i++) {
        char* payload = (char*)ls_alloc(48);
        payload[0] = (char)(i & 0x7f);
        sum += (unsigned char)payload[0];
        ls_release(payload);
    }
    return sum;
}

/* LsVec_String pushes share one handle, so this measures vector + retain cost. */
static unsigned long long sec_vec_string_push(size_t n)
{
    LsVec_String vec = ls_vec_string_new();
    LsString* shared = ls_str_cstr("x");
    size_t i;
    unsigned long long len;

    for (i = 0; i < n; i++) {
        ls_vec_string_push(&vec, shared);
    }
    len = (unsigned long long)vec.len;
    ls_vec_string_free(&vec);
    ls_release(shared);
    return len;
}

/* Mixed formatting: cstr + integer + char + float + cstr per iteration. */
static unsigned long long sec_fmt_mixed(size_t n)
{
    LsString* acc = ls_format_begin(64);
    size_t i;
    unsigned long long len;

    for (i = 0; i < n; i++) {
        acc = ls_format_push_cstr(acc, "v=");
        acc = ls_fmt_i64(acc, (int64_t)i);
        acc = ls_format_push_char(acc, ' ');
        acc = ls_fmt_f64(acc, (double)i * 0.5);
        acc = ls_format_push_cstr(acc, ";");
    }
    len = (unsigned long long)ls_str_len(acc);
    ls_release(acc);
    return len;
}

/* ------------------------------------------------------------------ */
/* peak-memory probes                                                 */
/* ------------------------------------------------------------------ */

static size_t parse_count(const char* text)
{
    char* end = NULL;
    unsigned long long value;

    if (text == NULL || *text == '\0') {
        return 0;
    }
    value = strtoull(text, &end, 10);
    if (end == NULL || *end != '\0' || value == 0) {
        return 0;
    }
    return (size_t)value;
}

static int peak_vec(size_t n)
{
    int64_t* data = NULL;
    size_t len = 0;
    size_t cap = 0;
    size_t i;

    for (i = 0; i < n; i++) {
        int64_t value = (int64_t)i;
        ls_vec_insert_bytes((void**)&data, &len, &cap, len, &value,
                            sizeof(value));
    }
    printf("peak-vec: n=%zu cap=%zu len=%zu checksum=%lld\n", n, cap, len,
           (long long)(data[0] + data[len - 1]));
    ls_vec_free_bytes(data);
    return 0;
}

static int peak_vec_cap(size_t n)
{
    int64_t* data = (int64_t*)ls_vec_alloc(n, sizeof(int64_t));
    size_t i;

    if (data == NULL) {
        fprintf(stderr, "bench_runtime: allocation failed\n");
        return 1;
    }
    for (i = 0; i < n; i++) {
        data[i] = (int64_t)i;
    }
    printf("peak-vec-cap: n=%zu checksum=%lld\n", n,
           (long long)(data[0] + data[n - 1]));
    ls_vec_free_bytes(data);
    return 0;
}

static int peak_str(size_t n)
{
    LsString* s = ls_str_new();
    size_t i;

    for (i = 0; i < n; i++) {
        s = ls_str_push_char(s, 'x');
    }
    printf("peak-str: n=%zu len=%zu\n", n, ls_str_len(s));
    ls_release(s);
    return 0;
}

static int peak_tokens(size_t n)
{
    LsString* acc = ls_format_begin(16);
    size_t i;

    for (i = 0; i < n; i++) {
        acc = ls_fmt_u64(acc, (uint64_t)(i % 1000));
        acc = ls_format_push_cstr(acc, ",");
    }
    printf("peak-tokens: n=%zu len=%zu\n", n, ls_str_len(acc));
    ls_release(acc);
    return 0;
}

/* ------------------------------------------------------------------ */

static int usage(const char* argv0)
{
    printf("usage: %s [--peak-vec N | --peak-vec-cap N | --peak-str N | "
           "--peak-tokens N]\n",
           argv0);
    return 0;
}

int main(int argc, char** argv)
{
    size_t n;

    if (ls_runtime_init(argc, argv) != 0) {
        fprintf(stderr, "bench_runtime: runtime init failed\n");
        return 1;
    }

    if (argc > 1) {
        if (strcmp(argv[1], "--help") == 0 || strcmp(argv[1], "-h") == 0) {
            usage(argv[0]);
            ls_runtime_shutdown();
            return 0;
        }
        if (strcmp(argv[1], "--peak-vec") == 0 && argc == 3) {
            n = parse_count(argv[2]);
            if (n == 0) {
                fprintf(stderr, "bench_runtime: bad count\n");
                ls_runtime_shutdown();
                return 2;
            }
            {
                int rc = peak_vec(n);
                ls_runtime_shutdown();
                return rc;
            }
        }
        if (strcmp(argv[1], "--peak-vec-cap") == 0 && argc == 3) {
            n = parse_count(argv[2]);
            if (n == 0) {
                fprintf(stderr, "bench_runtime: bad count\n");
                ls_runtime_shutdown();
                return 2;
            }
            {
                int rc = peak_vec_cap(n);
                ls_runtime_shutdown();
                return rc;
            }
        }
        if (strcmp(argv[1], "--peak-str") == 0 && argc == 3) {
            n = parse_count(argv[2]);
            if (n == 0) {
                fprintf(stderr, "bench_runtime: bad count\n");
                ls_runtime_shutdown();
                return 2;
            }
            {
                int rc = peak_str(n);
                ls_runtime_shutdown();
                return rc;
            }
        }
        if (strcmp(argv[1], "--peak-tokens") == 0 && argc == 3) {
            n = parse_count(argv[2]);
            if (n == 0) {
                fprintf(stderr, "bench_runtime: bad count\n");
                ls_runtime_shutdown();
                return 2;
            }
            {
                int rc = peak_tokens(n);
                ls_runtime_shutdown();
                return rc;
            }
        }
        usage(argv[0]);
        ls_runtime_shutdown();
        return 2;
    }

    printf("bench_runtime: best of 3 wall-clock runs per section\n");
    run_section("vec_push", (size_t)1000000, sec_vec_push);
    run_section("vec_grow", (size_t)1000000, sec_vec_grow);
    run_section("vec_exact", (size_t)1000000, sec_vec_exact);
    run_section("str_build", (size_t)1000000, sec_str_build);
    run_section("fmt_u64", (size_t)1000000, sec_fmt_u64);
    run_section("str_push_char", (size_t)1000000, sec_str_push_char);
    run_section("str_push_cstr", (size_t)200000, sec_str_push_cstr);
    run_section("alloc_release", (size_t)1000000, sec_alloc_release);
    run_section("vec_string_push", (size_t)200000, sec_vec_string_push);
    run_section("fmt_mixed", (size_t)20000, sec_fmt_mixed);

    ls_runtime_shutdown();
    return 0;
}
