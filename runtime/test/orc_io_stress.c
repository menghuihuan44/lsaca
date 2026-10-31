/* lsaca C17 runtime: standalone ORC / IO stress and regression program.
 *
 * This program is not part of the generated-code ABI. It is compiled directly
 * against the runtime translation units and checks the parts of the runtime
 * that the in-process smoke suite cannot reach from a single process:
 *
 *   1. Cycle collector stack safety: a 100k-node reference ring is collected
 *      and a 100k-node Box-style chain is destroyed without overflowing the C
 *      stack (iterative worklists, see lsaca_rt.c).
 *   2. Stale/double release diagnostics: a second ls_release() of a freed
 *      handle must print the runtime error and exit 101. In debug builds the
 *      live-handle registry must also catch a stale handle whose memory was
 *      already reused by a different live allocation.
 *   3. ls_read_line(): CRLF normalization, lone-CR preservation (Rust
 *      read_line semantics), EOF with/without a final newline, embedded NUL
 *      bytes, a 1 MiB single line, and invalid-UTF-8 panics with the exact
 *      message plus call-site location.
 *   4. ls_args_at(): valid arguments are materialized as owned Strings; the
 *      invalid-UTF-8 case is covered by the runtime unit tests.
 *   5. Output buffering: redirected stdout keeps every byte and flushes
 *      explicitly (ls_print_flush) and at shutdown.
 *   6. Measurements: header size and allocation/release, read_line and
 *      println costs, printed as INFO lines for the task report.
 *
 * Build (from the repository root, C:/msys64/ucrt64/bin on PATH):
 *   gcc -std=c17 -Wall -Wextra -Wpedantic -Werror -O2 -I runtime \
 *       runtime/test/orc_io_stress.c runtime/lsaca_rt.c \
 *       runtime/lsaca_rt_string.c runtime/lsaca_rt_vec.c \
 *       runtime/lsaca_rt_fmt.c runtime/lsaca_rt_map.c -lm \
 *       -o target/orc_io_stress.exe
 * and the same with -DLSACA_DEBUG -DLSACA_DEBUG_RC for the debug runtime.
 * Panic cases run in child processes (a panic exits the process); the parent
 * checks the child exit code and captured stderr.
 */

#include "lsaca_rt_internal.h"

#include <inttypes.h>
#include <stdarg.h>
#include <stdint.h>
#include <time.h>

#ifndef _WIN32
#include <sys/wait.h>
#endif

/* Test-harness hooks exported by lsaca_rt.c (not part of the public ABI). */
extern size_t ls_rt_test_live_allocations(void);
extern size_t ls_rt_test_candidate_threshold(void);
extern size_t ls_rt_test_set_candidate_threshold(size_t threshold);
extern void ls_rt_test_reset_stdin_buffer(void);

#define ORC_STRESS_GRAPH_NODES ((size_t)100000)
#define ORC_STRESS_MID_COLLECT_NODES ((size_t)5000)
#define ORC_STRESS_EVICTION_CHAIN_NODES ((size_t)300000)
#define ORC_STRESS_BENCH_LINES ((size_t)200000)
#define ORC_STRESS_BENCH_LINE_BYTES ((size_t)78)
#define ORC_STRESS_PRINTLN_COUNT ((size_t)20000)
#define ORC_STRESS_ALLOC_BENCH_ITERATIONS ((size_t)1000000)

static int64_t g_failures = 0;
static int64_t g_drops = 0;
static const char* g_exe = "orc_io_stress.exe";
static int g_child_serial = 0;

#define CHECK(cond)                                                           \
    do {                                                                      \
        if (!(cond)) {                                                        \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond);   \
            g_failures += 1;                                                  \
        }                                                                     \
    } while (0)

#define failf(...)                                                            \
    do {                                                                      \
        fprintf(stderr, "FAIL %s:%d: ", __FILE__, __LINE__);                  \
        fprintf(stderr, __VA_ARGS__);                                         \
        fprintf(stderr, "\n");                                                \
        g_failures += 1;                                                      \
    } while (0)

static void* xmalloc(size_t size) {
    void* pointer = malloc(size == 0 ? 1 : size);
    if (pointer == NULL) {
        fprintf(stderr, "orc_io_stress: out of memory\n");
        exit(2);
    }
    return pointer;
}

static void* xrealloc(void* pointer, size_t size) {
    void* next = realloc(pointer, size == 0 ? 1 : size);
    if (next == NULL) {
        fprintf(stderr, "orc_io_stress: out of memory\n");
        exit(2);
    }
    return next;
}

static double now_seconds(void) {
    struct timespec moment;
    timespec_get(&moment, TIME_UTC);
    return (double)moment.tv_sec + (double)moment.tv_nsec / 1000000000.0;
}

/* ---------------------------------------------------------------------- */
/* Small growable byte buffer (test infrastructure only).                 */
/* ---------------------------------------------------------------------- */

typedef struct Buffer {
    char* data;
    size_t len;
    size_t cap;
} Buffer;

static void buf_reserve(Buffer* buffer, size_t extra) {
    size_t needed = buffer->len + extra + 1;
    if (needed > buffer->cap) {
        size_t next_cap = buffer->cap == 0 ? 256 : buffer->cap;
        while (next_cap < needed) {
            next_cap *= 2;
        }
        buffer->data = (char*)xrealloc(buffer->data, next_cap);
        buffer->cap = next_cap;
    }
}

static void buf_append(Buffer* buffer, const void* bytes, size_t count) {
    if (count == 0) {
        return;
    }
    buf_reserve(buffer, count);
    memcpy(buffer->data + buffer->len, bytes, count);
    buffer->len += count;
    buffer->data[buffer->len] = '\0';
}

static void buf_printf(Buffer* buffer, const char* format, ...) {
    va_list args;
    int needed;
    va_start(args, format);
    needed = vsnprintf(NULL, 0, format, args);
    va_end(args);
    if (needed < 0) {
        fprintf(stderr, "orc_io_stress: vsnprintf failed\n");
        exit(2);
    }
    buf_reserve(buffer, (size_t)needed);
    va_start(args, format);
    (void)vsnprintf(buffer->data + buffer->len, buffer->cap - buffer->len,
                    format, args);
    va_end(args);
    buffer->len += (size_t)needed;
}

static void buf_free(Buffer* buffer) {
    free(buffer->data);
    buffer->data = NULL;
    buffer->len = 0;
    buffer->cap = 0;
}

static int buf_equals(const Buffer* buffer, const void* bytes, size_t count) {
    return buffer->len == count &&
           (count == 0 || memcmp(buffer->data, bytes, count) == 0);
}

static int buf_contains(const Buffer* buffer, const char* needle) {
    size_t needle_len = strlen(needle);
    size_t index;
    if (needle_len == 0) {
        return 1;
    }
    if (buffer->len < needle_len) {
        return 0;
    }
    for (index = 0; index + needle_len <= buffer->len; index += 1) {
        if (memcmp(buffer->data + index, needle, needle_len) == 0) {
            return 1;
        }
    }
    return 0;
}

static int read_file_into(const char* path, Buffer* out) {
    FILE* file = fopen(path, "rb");
    char chunk[4096];
    if (file == NULL) {
        return 0;
    }
    for (;;) {
        size_t count = fread(chunk, 1, sizeof chunk, file);
        if (count == 0) {
            break;
        }
        buf_append(out, chunk, count);
    }
    if (fclose(file) != 0) {
        return 0;
    }
    return 1;
}

static int write_file_bytes(const char* path, const void* bytes, size_t count) {
    FILE* file = fopen(path, "wb");
    if (file == NULL) {
        return 0;
    }
    if (count != 0 && fwrite(bytes, 1, count, file) != count) {
        (void)fclose(file);
        return 0;
    }
    return fclose(file) == 0;
}

static void write_result_file(const char* path, const char* format, ...) {
    va_list args;
    FILE* file = fopen(path, "wb");
    if (file == NULL) {
        return;
    }
    va_start(args, format);
    (void)vfprintf(file, format, args);
    va_end(args);
    (void)fclose(file);
}

/* ---------------------------------------------------------------------- */
/* Child process runner: a panic path exits the process, so every panic    */
/* case runs in a child and the parent checks exit code + captured stderr. */
/* ---------------------------------------------------------------------- */

static int run_child(const char* mode, const char* arg1, const char* arg2,
                     const char* arg3, Buffer* out, Buffer* err) {
    Buffer command = {NULL, 0, 0};
    char out_path[512];
    char err_path[512];
    int status;
    int ok;

    g_child_serial += 1;
    (void)snprintf(out_path, sizeof out_path, "orc_io_stress_%s_%d.out",
                   mode, g_child_serial);
    (void)snprintf(err_path, sizeof err_path, "orc_io_stress_%s_%d.err",
                   mode, g_child_serial);
#ifdef _WIN32
    /* cmd.exe strips the first and last quote of a /c command line, so wrap
     * the whole command in one extra quoted layer. */
    buf_printf(&command, "cmd /c \"");
#endif
    buf_printf(&command, "\"%.2000s\" %.200s", g_exe, mode);
    if (arg1 != NULL) {
        buf_printf(&command, " \"%.900s\"", arg1);
    }
    if (arg2 != NULL) {
        buf_printf(&command, " \"%.900s\"", arg2);
    }
    if (arg3 != NULL) {
        buf_printf(&command, " \"%.900s\"", arg3);
    }
    buf_printf(&command, " > \"%.400s\" 2> \"%.400s\"", out_path, err_path);
#ifdef _WIN32
    buf_printf(&command, "\"");
#endif
    status = system(command.data);
    buf_free(&command);
#ifdef _WIN32
    /* Windows' system() returns the child exit code directly. */
#else
    if (status != -1) {
        if (WIFEXITED(status)) {
            status = WEXITSTATUS(status);
        } else {
            status = 128;
        }
    }
#endif
    ok = read_file_into(out_path, out);
    if (!ok) {
        failf("could not read child stdout %s", out_path);
    }
    ok = read_file_into(err_path, err);
    if (!ok) {
        failf("could not read child stderr %s", err_path);
    }
    (void)remove(out_path);
    (void)remove(err_path);
    return status;
}

/* ---------------------------------------------------------------------- */
/* ORC graph helpers mirroring generated drop/trace functions.            */
/* ---------------------------------------------------------------------- */

typedef struct StressNode {
    struct StressNode* child; /* owned managed handle */
    struct StressNode* extra; /* owned managed handle */
    void* name;               /* owned unmanaged (ls_alloc) payload */
    int64_t value;
} StressNode;

static void stress_node_drop(void* payload) {
    StressNode* node = (StressNode*)payload;
    void* name = node->name;
    void* child = node->child;
    void* extra = node->extra;

    node->name = NULL;
    node->child = NULL;
    node->extra = NULL;
    if (name != NULL) {
        ls_release(name);
    }
    if (child != NULL) {
        ls_release(child);
    }
    if (extra != NULL) {
        ls_release(extra);
    }
    g_drops += 1;
}

static void stress_node_trace(const void* payload, LsTraceVisit visit) {
    const StressNode* node = (const StressNode*)payload;
    if (node->child != NULL) {
        visit(node->child);
    }
    if (node->extra != NULL) {
        visit(node->extra);
    }
}

static StressNode* stress_node_new(void) {
    return (StressNode*)ls_alloc_managed(sizeof(StressNode), stress_node_drop,
                                         stress_node_trace);
}

static void stress_node_set_name(StressNode* node, size_t bytes, char fill) {
    char* name = (char*)ls_alloc(bytes);
    memset(name, fill, bytes);
    node->name = name;
}

/* ---------------------------------------------------------------------- */
/* Test 1: 100k-node ring collection and 100k-node chain destruction.     */
/* ---------------------------------------------------------------------- */

static void test_ring_100k(void) {
    const size_t count = ORC_STRESS_GRAPH_NODES;
    size_t base = ls_rt_test_live_allocations();
    size_t previous_threshold = ls_rt_test_set_candidate_threshold(SIZE_MAX);
    int64_t drops = g_drops;
    StressNode** nodes = (StressNode**)xmalloc(count * sizeof(*nodes));
    size_t index;

    for (index = 0; index < count; index += 1) {
        nodes[index] = stress_node_new();
        nodes[index]->value = (int64_t)index;
    }
    for (index = 0; index < count; index += 1) {
        StressNode* next = nodes[(index + 1) % count];
        nodes[index]->child = (StressNode*)ls_retain(next);
    }
    CHECK(ls_rt_test_live_allocations() == base + count);
    for (index = 0; index < count; index += 1) {
        ls_release(nodes[index]); /* array handles: refcount 2 -> 1 */
    }
    CHECK(ls_rt_test_live_allocations() == base + count);
    CHECK(ls_rt_test_candidate_threshold() == SIZE_MAX);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_drops - drops == (int64_t)count);
    free(nodes);
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
    printf("INFO ring_100k collected nodes=%zu drops=%" PRId64 "\n", count,
           (int64_t)(g_drops - drops));
}

static void test_ring_mid_release_collection(void) {
    const size_t count = ORC_STRESS_MID_COLLECT_NODES;
    size_t base = ls_rt_test_live_allocations();
    size_t previous_threshold = ls_rt_test_set_candidate_threshold(512);
    int64_t drops = g_drops;
    StressNode** nodes = (StressNode**)xmalloc(count * sizeof(*nodes));
    size_t index;

    for (index = 0; index < count; index += 1) {
        nodes[index] = stress_node_new();
    }
    for (index = 0; index < count; index += 1) {
        StressNode* next = nodes[(index + 1) % count];
        nodes[index]->child = (StressNode*)ls_retain(next);
    }
    /* Releasing every array handle crosses the candidate threshold several
     * times, so the collector runs while the ring is still partly referenced
     * from the outside. It must keep the live nodes and only free the ring
     * after the last external reference is gone. */
    for (index = 0; index < count; index += 1) {
        ls_release(nodes[index]);
    }
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_drops - drops == (int64_t)count);
    free(nodes);
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
    printf("INFO ring_mid_collect nodes=%zu drops=%" PRId64 "\n", count,
           (int64_t)(g_drops - drops));
}

static void run_chain_destroy(size_t count, const char* label) {
    size_t base = ls_rt_test_live_allocations();
    size_t previous_threshold = ls_rt_test_set_candidate_threshold(SIZE_MAX);
    int64_t drops = g_drops;
    StressNode** nodes = (StressNode**)xmalloc(count * sizeof(*nodes));
    size_t index;

    for (index = 0; index < count; index += 1) {
        nodes[index] = stress_node_new();
        stress_node_set_name(nodes[index], 16, (char)('a' + (int)(index % 26)));
    }
    for (index = 0; index + 1 < count; index += 1) {
        nodes[index]->child = (StressNode*)ls_retain(nodes[index + 1]);
    }
    CHECK(ls_rt_test_live_allocations() == base + 2 * count);
    for (index = 1; index < count; index += 1) {
        ls_release(nodes[index]); /* leave only the parent link + array handle */
    }
    CHECK(ls_rt_test_live_allocations() == base + 2 * count);
    /* Releasing the head cascades through every node and every name.
     * The drop cascade must not grow the C stack. */
    ls_release(nodes[0]);
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_drops - drops == (int64_t)count);
    free(nodes);
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
    printf("INFO chain_%s destroyed nodes=%zu drops=%" PRId64 "\n", label, count,
           (int64_t)(g_drops - drops));
}

static void test_chain_100k_destroy(void) {
    run_chain_destroy(ORC_STRESS_GRAPH_NODES, "100k");
}

#if defined(LS_RT_LEAK_TRACKING)
/* Debug builds keep destroyed headers in a bounded quarantine so that stale
 * handles are diagnosed. Crossing the window must evict the oldest reserved
 * blocks without touching live state. This chain is longer than the window. */
static void test_chain_quarantine_eviction(void) {
    run_chain_destroy(ORC_STRESS_EVICTION_CHAIN_NODES, "eviction");
}
#endif

static void test_small_cycle(void) {
    size_t base = ls_rt_test_live_allocations();
    int64_t drops = g_drops;
    StressNode* first = stress_node_new();
    StressNode* second = stress_node_new();

    first->child = (StressNode*)ls_retain(second);
    second->child = (StressNode*)ls_retain(first);
    CHECK(ls_rt_test_live_allocations() == base + 2);
    ls_release(first);
    ls_release(second);
    CHECK(ls_rt_test_live_allocations() == base + 2);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_drops - drops == 2);
}

/* ---------------------------------------------------------------------- */
/* Test 1b: interned-literal payloads (ls_rt_alloc_immortal contract):     */
/* never counted, never destroyed, debug registry accepts the handle.     */
/* ---------------------------------------------------------------------- */

static void test_immortal_allocations(void) {
    size_t base = ls_rt_test_live_allocations();
    char* payload = (char*)ls_rt_alloc_immortal(64);

    CHECK(payload != NULL);
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(ls_refcount(payload) == 1);
    ls_retain(payload);
    CHECK(ls_refcount(payload) == 2);
    ls_release(payload);
    CHECK(ls_refcount(payload) == 1);
    /* The final "static" reference is intentionally kept: releasing the last
     * handle must not destroy or free the interned literal. */
    ls_release(payload);
    CHECK(ls_refcount(payload) == 1);
    payload[0] = 'i';
    payload[63] = 'x';
    CHECK(payload[0] == 'i' && payload[63] == 'x');
    CHECK(ls_rt_test_live_allocations() == base);
}

static void test_acyclic_destruction(void) {
    size_t base = ls_rt_test_live_allocations();
    int64_t drops = g_drops;
    StressNode* parent = stress_node_new();
    StressNode* child = stress_node_new();

    stress_node_set_name(parent, 8, 'p');
    stress_node_set_name(child, 8, 'c');
    parent->child = (StressNode*)ls_retain(child);
    CHECK(ls_rt_test_live_allocations() == base + 4);
    ls_release(parent);
    CHECK(ls_rt_test_live_allocations() == base + 2);
    ls_release(child);
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_drops - drops == 2);
}

/* ---------------------------------------------------------------------- */
/* Test 2: stale/double release diagnostics.                              */
/* ---------------------------------------------------------------------- */

static void test_release_diagnostics(void) {
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    int status;

    status = run_child("double-release", NULL, NULL, NULL, &out, &err);
    CHECK(status == 101);
    CHECK(buf_contains(&err,
                       "release of an invalid or already released "
                       "reference-counted object"));
    buf_free(&out);
    buf_free(&err);

#if defined(LS_RT_LEAK_TRACKING)
    /* Debug builds: the live-handle registry must catch a stale handle even
     * when a fresh allocation reused the freed block. */
    status = run_child("stale-release", NULL, NULL, NULL, &out, &err);
    CHECK(status == 101);
    CHECK(buf_contains(&err,
                       "release of an invalid or already released "
                       "reference-counted object"));
    buf_free(&out);
    buf_free(&err);
    printf("INFO stale_handle_registry debug=enabled\n");
#else
    printf("INFO stale_handle_registry debug=disabled (release best-effort)\n");
#endif
}

/* ---------------------------------------------------------------------- */
/* Test 3: read_line line splitting, CRLF/CR/NUL/EOF/UTF-8 semantics.     */
/* ---------------------------------------------------------------------- */

typedef struct LineRef {
    const unsigned char* data;
    size_t len;
} LineRef;

static void build_expected_lines(Buffer* expected, const LineRef* lines,
                                 size_t count) {
    size_t index;
    for (index = 0; index < count; index += 1) {
        size_t byte_index;
        buf_printf(expected, "LINE %zu ", lines[index].len);
        for (byte_index = 0; byte_index < lines[index].len; byte_index += 1) {
            buf_printf(expected, "%02x", (unsigned int)lines[index].data[byte_index]);
        }
        buf_append(expected, "\n", 1);
    }
}

static void run_read_line_case(const char* name, const void* bytes,
                               size_t length, const LineRef* lines,
                               size_t line_count) {
    char path[256];
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    Buffer expected = {NULL, 0, 0};
    int status;

    (void)snprintf(path, sizeof path, "orc_io_stress_input_%s.bin", name);
    CHECK(write_file_bytes(path, bytes, length));
    build_expected_lines(&expected, lines, line_count);
    status = run_child("readline-case", path, NULL, NULL, &out, &err);
    CHECK(status == 0);
    if (!buf_equals(&out, expected.data, expected.len)) {
        failf("read_line case %s output mismatch (got %zu bytes, expected %zu)",
              name, out.len, expected.len);
    }
    if (err.len != 0) {
        failf("read_line case %s wrote unexpected stderr", name);
    }
    (void)remove(path);
    buf_free(&out);
    buf_free(&err);
    buf_free(&expected);
}

static void test_read_line_cases(void) {
    static const unsigned char crlf[] = "alpha\r\nbeta\r\ngamma";
    static const unsigned char crlf_a[] = "alpha";
    static const unsigned char crlf_b[] = "beta";
    static const unsigned char crlf_c[] = "gamma";
    static const LineRef crlf_lines[3] = {
        {crlf_a, 5}, {crlf_b, 4}, {crlf_c, 5}};

    static const unsigned char lone_cr[] = "abc\r";
    static const unsigned char lone_cr_line[] = "abc\r";
    static const LineRef lone_cr_lines[1] = {{lone_cr_line, 4}};

    static const unsigned char nul_bytes[] = {'a', 0, 'b', '\n', 'x', 0, 'y', '\n'};
    static const unsigned char nul_a[] = {'a', 0, 'b'};
    static const unsigned char nul_b[] = {'x', 0, 'y'};
    static const LineRef nul_lines[2] = {{nul_a, 3}, {nul_b, 3}};

    static const unsigned char mixed_cr[] = "a\rb\r\n";
    static const unsigned char mixed_line[] = "a\rb";
    static const LineRef mixed_lines[1] = {{mixed_line, 3}};

    static const unsigned char empty_bytes[1] = {0};
    static const unsigned char emptyline_bytes[] = "\n\n";
    static const unsigned char empty_line[1] = {0};
    static const LineRef emptyline_lines[2] = {{empty_line, 0}, {empty_line, 0}};

    run_read_line_case("crlf", crlf, sizeof crlf - 1, crlf_lines, 3);
    run_read_line_case("lonecr", lone_cr, sizeof lone_cr - 1, lone_cr_lines, 1);
    run_read_line_case("nul", nul_bytes, sizeof nul_bytes, nul_lines, 2);
    run_read_line_case("mixedcr", mixed_cr, sizeof mixed_cr - 1, mixed_lines, 1);
    run_read_line_case("empty", empty_bytes, 0, NULL, 0);
    run_read_line_case("emptyline", emptyline_bytes, sizeof emptyline_bytes - 1,
                       emptyline_lines, 2);

    {
        /* A 1 MiB single line followed by a short final line without newline. */
        const size_t huge_length = (size_t)1 << 20;
        const size_t total = huge_length + 1 + 4;
        unsigned char* huge = (unsigned char*)xmalloc(total);
        LineRef lines[2];
        size_t index;
        for (index = 0; index < huge_length; index += 1) {
            huge[index] = 'A';
        }
        huge[huge_length] = '\n';
        memcpy(huge + huge_length + 1, "tail", 4);
        lines[0].data = huge;
        lines[0].len = huge_length;
        lines[1].data = (const unsigned char*)"tail";
        lines[1].len = 4;
        run_read_line_case("huge", huge, total, lines, 2);
        free(huge);
    }
}

static void test_read_line_invalid_utf8(void) {
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    char path[256];
    int status;

    /* Truncated two-byte sequence: "A" 0xC3 "\n". */
    static const unsigned char truncated[] = {'A', 0xC3, '\n'};
    (void)snprintf(path, sizeof path, "orc_io_stress_input_invalid.bin");
    CHECK(write_file_bytes(path, truncated, sizeof truncated));
    status = run_child("readline-invalid", path, NULL, NULL, &out, &err);
    CHECK(status == 101);
    CHECK(buf_contains(&err, "stream did not contain valid UTF-8"));
    CHECK(buf_contains(&err, "stress.ls:7:3"));
    (void)remove(path);
    buf_free(&out);
    buf_free(&err);

    /* Overlong encoding 0xC0 0x80 is invalid UTF-8 as well. */
    {
        static const unsigned char overlong[] = {0xC0, 0x80, '\n'};
        CHECK(write_file_bytes(path, overlong, sizeof overlong));
        status = run_child("readline-invalid", path, NULL, NULL, &out, &err);
        CHECK(status == 101);
        CHECK(buf_contains(&err, "stream did not contain valid UTF-8"));
        (void)remove(path);
        buf_free(&out);
        buf_free(&err);
    }
}

/* ---------------------------------------------------------------------- */
/* Test 3b: read_file UTF-8 validation and binary round-trip.             */
/* ---------------------------------------------------------------------- */

static void test_read_file_cases(void) {
    static const unsigned char invalid_bytes[] = {'A', 0xFF, 'B'};
    static const unsigned char valid_bytes[] = {'a', 0, 'b', 0xC3, 0xA9,
                                                '\n', 'z'};
    static const char invalid_path[] = "orc_io_stress_readfile_invalid.bin";
    static const char valid_path[] = "orc_io_stress_readfile_valid.bin";
    static const char missing_path[] = "orc_io_stress_readfile_missing.bin";
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    int status;

    CHECK(write_file_bytes(invalid_path, invalid_bytes, sizeof invalid_bytes));
    CHECK(write_file_bytes(valid_path, valid_bytes, sizeof valid_bytes));
    (void)remove(missing_path);
    status = run_child("readfile-case", invalid_path, valid_path, missing_path,
                       &out, &err);
    CHECK(status == 0);
    CHECK(buf_contains(&out,
        "READFILE invalid tag=1 msg=stream did not contain valid UTF-8\n"));
    CHECK(buf_contains(&out,
        "READFILE valid tag=0 len=7 hex=610062c3a90a7a\n"));
    CHECK(buf_contains(&out, "READFILE missing tag=1 msg="));
    CHECK(buf_contains(&out, "(os error 2)"));
    if (err.len != 0) {
        failf("read_file case wrote unexpected stderr");
    }
    (void)remove(invalid_path);
    (void)remove(valid_path);
    buf_free(&out);
    buf_free(&err);
}

/* ---------------------------------------------------------------------- */
/* Test 4: ls_args_at materializes owned valid arguments.                 */
/* ---------------------------------------------------------------------- */

static void test_args(void) {
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    int status = run_child("args-case", "alpha", "beta", NULL, &out, &err);
    /* argv[1] is the child mode itself, then the two extra arguments. */
    static const char expected[] =
        "ARGS 3\n"
        "ARG 9 617267732d63617365\n"
        "ARG 5 616c706861\n"
        "ARG 4 62657461\n";
    CHECK(status == 0);
    if (!buf_equals(&out, expected, sizeof expected - 1)) {
        failf("args output mismatch (got %zu bytes, expected %zu)", out.len,
              sizeof expected - 1);
    }
    if (err.len != 0) {
        failf("args case wrote unexpected stderr");
    }
    buf_free(&out);
    buf_free(&err);
}

/* ---------------------------------------------------------------------- */
/* Test 5: redirected stdout buffering keeps every byte.                  */
/* ---------------------------------------------------------------------- */

static void test_buffered_output(void) {
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    static const char expected[] = "no-newline-tail\nline\nlast";
    int status = run_child("buffered-output", NULL, NULL, NULL, &out, &err);

    CHECK(status == 0);
    if (!buf_equals(&out, expected, sizeof expected - 1)) {
        failf("buffered output mismatch (got %zu bytes, expected %zu)", out.len,
              sizeof expected - 1);
    }
    if (err.len != 0) {
        failf("buffered output wrote unexpected stderr");
    }
    buf_free(&out);
    buf_free(&err);
}

/* ---------------------------------------------------------------------- */
/* Test 5b: invalid UTF-8 in argv panics with a located message.          */
/* ---------------------------------------------------------------------- */

static void test_args_invalid(void) {
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    int status = run_child("args-invalid", NULL, NULL, NULL, &out, &err);

    CHECK(status == 101);
    CHECK(buf_contains(&err, "arguments contain invalid UTF-8"));
    CHECK(buf_contains(&err, "stress.ls:11:2"));
    buf_free(&out);
    buf_free(&err);
}

#if defined(LS_RT_LEAK_TRACKING)
/* ---------------------------------------------------------------------- */
/* Test 6: shutdown leak report wording is exact and deterministic.       */
/* ---------------------------------------------------------------------- */

static void test_leak_report(void) {
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    static const char expected[] =
        "lsaca: warning: 2 reference-counted allocation(s) leaked\n"
        "lsaca:   leaked allocation of 24 bytes\n"
        "lsaca:   leaked allocation of 8 bytes\n";
    int status = run_child("leak-report", NULL, NULL, NULL, &out, &err);

    CHECK(status == 0);
    if (!buf_equals(&err, expected, sizeof expected - 1)) {
        failf("leak report mismatch (got %zu bytes, expected %zu)", err.len,
              sizeof expected - 1);
    }
    if (out.len != 0) {
        failf("leak report wrote unexpected stdout");
    }
    buf_free(&out);
    buf_free(&err);
}
#endif

/* ---------------------------------------------------------------------- */
/* Measurements.                                                          */
/* ---------------------------------------------------------------------- */

static void measure_alloc(void) {
    const size_t iterations = ORC_STRESS_ALLOC_BENCH_ITERATIONS;
    volatile void* sink = NULL;
    double start;
    double elapsed;
    size_t index;

    printf("INFO header_bytes=%zu header_space=%zu tracking=%d\n",
           sizeof(LsRtHeader), LS_RT_HEADER_SPACE,
#if defined(LS_RT_LEAK_TRACKING)
           1
#else
           0
#endif
    );
#if defined(LS_RT_LEAK_TRACKING)
    CHECK(sizeof(LsRtHeader) <= 64);
    CHECK(LS_RT_HEADER_SPACE <= 64);
#else
    CHECK(sizeof(LsRtHeader) <= 40);
    CHECK(LS_RT_HEADER_SPACE <= 48);
#endif
    CHECK(sizeof(LsRtHeader) < 72); /* smaller than the v1.5 header */

    start = now_seconds();
    for (index = 0; index < iterations; index += 1) {
        sink = ls_alloc(64);
        ls_release((void*)sink);
    }
    elapsed = now_seconds() - start;
    printf("INFO alloc_release ns_per_op=%.1f iterations=%zu\n",
           elapsed * 1e9 / (double)iterations, iterations);

    start = now_seconds();
    for (index = 0; index < iterations; index += 1) {
        void* block = malloc(64);
        if (block == NULL) {
            failf("malloc baseline allocation failed");
            break;
        }
        ((volatile char*)block)[0] = 0; /* keep the pair from being elided */
        sink = block;
        free(block);
    }
    elapsed = now_seconds() - start;
    printf("INFO malloc_free ns_per_op=%.1f iterations=%zu\n",
           elapsed * 1e9 / (double)iterations, iterations);
    (void)sink;
}

static void create_bench_lines_file(const char* path) {
    FILE* file = fopen(path, "wb");
    char line[ORC_STRESS_BENCH_LINE_BYTES];
    size_t index;
    if (file == NULL) {
        failf("could not create %s", path);
        return;
    }
    memset(line, 'x', sizeof line);
    for (index = 0; index < ORC_STRESS_BENCH_LINES; index += 1) {
        if (fwrite(line, 1, sizeof line, file) != sizeof line ||
            fputc('\n', file) == EOF) {
            failf("could not write %s", path);
            break;
        }
    }
    if (fclose(file) != 0) {
        failf("could not close %s", path);
    }
}

static double parse_metric(const char* path, const char* key) {
    Buffer result = {NULL, 0, 0};
    double value = -1.0;
    const char* found;
    if (!read_file_into(path, &result)) {
        return -1.0;
    }
    buf_append(&result, "", 0);
    found = strstr(result.data != NULL ? result.data : "", key);
    if (found != NULL) {
        (void)sscanf(found + strlen(key), "%lf", &value);
    }
    buf_free(&result);
    return value;
}

static void measure_io(void) {
    const char* lines_path = "orc_io_stress_bench_lines.bin";
    const char* result_path = "orc_io_stress_result.txt";
    Buffer out = {NULL, 0, 0};
    Buffer err = {NULL, 0, 0};
    int status;

    create_bench_lines_file(lines_path);
    status = run_child("bench-readline", lines_path, result_path, NULL, &out,
                       &err);
    if (status != 0) {
        failf("read_line benchmark failed with exit %d", status);
    } else {
        printf("INFO read_line ns_per_line=%.1f lines=%zu\n",
               parse_metric(result_path, "ns_per_line="),
               ORC_STRESS_BENCH_LINES);
    }
    (void)remove(lines_path);
    (void)remove(result_path);
    buf_free(&out);
    buf_free(&err);

    status = run_child("bench-println", result_path, NULL, NULL, &out, &err);
    if (status != 0) {
        failf("println benchmark failed with exit %d", status);
    } else {
        printf("INFO println ns_per_line=%.1f count=%zu\n",
               parse_metric(result_path, "ns_per_line="),
               ORC_STRESS_PRINTLN_COUNT);
    }
    (void)remove(result_path);
    buf_free(&out);
    buf_free(&err);
}

/* ---------------------------------------------------------------------- */
/* Child-process modes.                                                   */
/* ---------------------------------------------------------------------- */

static int child_readline_case(const char* path) {
    LsOpt_String line;
    FILE* redirected = freopen(path, "rb", stdin);
    if (redirected == NULL) {
        fprintf(stderr, "freopen failed for %s\n", path);
        return 4;
    }
    ls_rt_test_reset_stdin_buffer();
    for (;;) {
        size_t index;
        line = ls_read_line_at("stress.ls", 1, 1);
        if (line.tag == 0) {
            break;
        }
        printf("LINE %zu ", line.u.some->len);
        for (index = 0; index < line.u.some->len; index += 1) {
            printf("%02x", (unsigned int)(unsigned char)line.u.some->data[index]);
        }
        printf("\n");
        ls_release(line.u.some);
    }
    return 0;
}

static int child_readline_invalid(const char* path) {
    LsOpt_String line;
    FILE* redirected = freopen(path, "rb", stdin);
    if (redirected == NULL) {
        fprintf(stderr, "freopen failed for %s\n", path);
        return 4;
    }
    ls_rt_test_reset_stdin_buffer();
    line = ls_read_line_at("stress.ls", 7, 3);
    fprintf(stderr, "invalid UTF-8 read_line returned tag=%d\n", line.tag);
    return 3;
}

/* Read-file cases: valid UTF-8 content round-trips byte for byte (including
 * embedded NUL), invalid UTF-8 yields Err instead of a panic, and OS errors
 * surface as Err messages with the "(os error N)" suffix. */
static void print_read_file_result(const char* label,
                                   LsRes_String_String result) {
    const LsString* value = result.tag == 0 ? result.u.ok : result.u.err;
    size_t index;
    if (result.tag == 0) {
        printf("READFILE %s tag=0 len=%zu hex=", label, value->len);
        for (index = 0; index < value->len; index += 1) {
            printf("%02x", (unsigned int)(unsigned char)value->data[index]);
        }
        printf("\n");
    } else {
        printf("READFILE %s tag=1 msg=", label);
        if (value->len != 0) {
            (void)fwrite(value->data, 1, value->len, stdout);
        }
        printf("\n");
    }
    ls_release((void*)value);
}

static int child_read_file_cases(const char* invalid_path,
                                 const char* valid_path,
                                 const char* missing_path) {
    LsString* path;
    LsRes_String_String result;

    path = ls_str_cstr(invalid_path);
    result = ls_read_file(path);
    ls_release(path);
    print_read_file_result("invalid", result);

    path = ls_str_cstr(valid_path);
    result = ls_read_file(path);
    ls_release(path);
    print_read_file_result("valid", result);

    path = ls_str_cstr(missing_path);
    result = ls_read_file(path);
    ls_release(path);
    print_read_file_result("missing", result);
    return 0;
}

static int child_args_case(void) {
    LsVec_String args = ls_args_at("stress.ls", 9, 5);
    size_t index;
    printf("ARGS %zu\n", args.len);
    for (index = 0; index < args.len; index += 1) {
        LsString* value = args.data[index];
        size_t byte_index;
        printf("ARG %zu ", value->len);
        for (byte_index = 0; byte_index < value->len; byte_index += 1) {
            printf("%02x", (unsigned int)(unsigned char)value->data[byte_index]);
        }
        printf("\n");
    }
    ls_vec_string_free(&args);
    return 0;
}

/* Invalid-UTF-8 argv materialization: ls_args_at must panic Rust-style.
 * The fake argv bytes are installed through ls_runtime_init so the runtime's
 * stored argument vector is exercised directly, without OS argv encoding. */
static int child_args_invalid(void) {
    static char program[] = "prog";
    static char invalid_argument[] = {'b', 'a', 'd', (char)0xC3, 0};
    char* fake_argv[3];
    LsVec_String args;

    fake_argv[0] = program;
    fake_argv[1] = invalid_argument;
    fake_argv[2] = NULL;
    (void)ls_runtime_init(2, fake_argv);
    args = ls_args_at("stress.ls", 11, 2);
    ls_vec_string_free(&args);
    fprintf(stderr, "invalid args returned normally\n");
    return 3;
}

static int child_buffered_output(void) {
    ls_print_cstr("no-newline");
    ls_print_cstr("-tail");
    ls_print_flush();
    ls_println_cstr("");
    ls_println_cstr("line");
    ls_print_cstr("last");
    return 0;
}

#if defined(LS_RT_LEAK_TRACKING)
static int child_leak_report(void) {
    /* Deliberately abandoned allocations: shutdown must report both, newest
     * first, with the exact v1.5-compatible wording. */
    void* first = ls_alloc(8);
    void* second = ls_alloc(24);
    (void)first;
    (void)second;
    ls_runtime_shutdown();
    return 0;
}
#endif

static int child_bench_readline(const char* input_path, const char* result_path) {
    FILE* redirected = freopen(input_path, "rb", stdin);
    size_t lines = 0;
    double start;
    double elapsed;
    LsOpt_String line;
    if (redirected == NULL) {
        fprintf(stderr, "freopen failed for %s\n", input_path);
        return 4;
    }
    ls_rt_test_reset_stdin_buffer();
    start = now_seconds();
    for (;;) {
        line = ls_read_line_at("bench", 1, 1);
        if (line.tag == 0) {
            break;
        }
        lines += 1;
        ls_release(line.u.some);
    }
    elapsed = now_seconds() - start;
    write_result_file(result_path,
                      "lines=%zu ms=%.3f ns_per_line=%.1f\n", lines,
                      elapsed * 1000.0,
                      lines == 0 ? 0.0 : elapsed * 1e9 / (double)lines);
    return 0;
}

static int child_bench_println(const char* result_path) {
    size_t index;
    double start;
    double elapsed;
    start = now_seconds();
    for (index = 0; index < ORC_STRESS_PRINTLN_COUNT; index += 1) {
        ls_println_cstr("hello world");
    }
    elapsed = now_seconds() - start;
    write_result_file(result_path, "count=%zu ms=%.3f ns_per_line=%.1f\n",
                      ORC_STRESS_PRINTLN_COUNT, elapsed * 1000.0,
                      elapsed * 1e9 / (double)ORC_STRESS_PRINTLN_COUNT);
    return 0;
}

static int is_child_mode(const char* mode) {
    static const char* const modes[] = {
        "double-release", "stale-release", "readline-case", "readline-invalid",
        "args-case",       "buffered-output", "leak-report",
        "args-invalid",    "bench-readline", "bench-println",
        "readfile-case"};
    size_t index;
    for (index = 0; index < sizeof modes / sizeof modes[0]; index += 1) {
        if (strcmp(mode, modes[index]) == 0) {
            return 1;
        }
    }
    return 0;
}

static int run_child_mode(const char* mode, int argc, char** argv) {
    if (strcmp(mode, "double-release") == 0) {
        void* payload = ls_alloc(64);
        ls_release(payload);
        ls_release(payload);
        fprintf(stderr, "double release returned normally\n");
        return 3;
    }
    if (strcmp(mode, "stale-release") == 0) {
        void* payload = ls_alloc(64);
        void* reused;
        ls_release(payload);
        reused = ls_alloc(64); /* likely reuses the freed block */
        (void)reused;
        ls_release(payload);
        fprintf(stderr, "stale release returned normally (reused=%p)\n", reused);
        return 3;
    }
    if (strcmp(mode, "buffered-output") == 0) {
        return child_buffered_output();
    }
#if defined(LS_RT_LEAK_TRACKING)
    if (strcmp(mode, "leak-report") == 0) {
        return child_leak_report();
    }
#endif
    if (strcmp(mode, "args-invalid") == 0) {
        return child_args_invalid();
    }
    if (strcmp(mode, "readfile-case") == 0) {
        if (argc < 5) {
            fprintf(stderr, "readfile-case needs three paths\n");
            return 4;
        }
        return child_read_file_cases(argv[2], argv[3], argv[4]);
    }
    if (argc < 3) {
        fprintf(stderr, "mode %s needs an argument\n", mode);
        return 4;
    }
    if (strcmp(mode, "readline-case") == 0) {
        return child_readline_case(argv[2]);
    }
    if (strcmp(mode, "readline-invalid") == 0) {
        return child_readline_invalid(argv[2]);
    }
    if (strcmp(mode, "args-case") == 0) {
        return child_args_case();
    }
    if (strcmp(mode, "bench-readline") == 0) {
        if (argc < 4) {
            fprintf(stderr, "bench-readline needs input and result paths\n");
            return 4;
        }
        return child_bench_readline(argv[2], argv[3]);
    }
    if (strcmp(mode, "bench-println") == 0) {
        return child_bench_println(argv[2]);
    }
    fprintf(stderr, "unknown mode %s\n", mode);
    return 4;
}

/* ---------------------------------------------------------------------- */

int main(int argc, char** argv) {
    int failures;

    if (argc > 0 && argv[0] != NULL) {
        g_exe = argv[0];
    }
    (void)ls_runtime_init(argc, argv);
    if (argc > 1 && argv[1] != NULL) {
        int child_status;
        if (is_child_mode(argv[1])) {
            child_status = run_child_mode(argv[1], argc, argv);
            ls_runtime_shutdown();
            return child_status;
        }
    }

    /* Deterministic: no randomness, no timestamps in assertions. */
    test_small_cycle();
    test_acyclic_destruction();
    test_immortal_allocations();
    test_ring_100k();
    test_ring_mid_release_collection();
    test_chain_100k_destroy();
#if defined(LS_RT_LEAK_TRACKING)
    test_chain_quarantine_eviction();
#endif
    test_release_diagnostics();
    test_read_line_cases();
    test_read_line_invalid_utf8();
    test_read_file_cases();
    test_args();
    test_args_invalid();
    test_buffered_output();
#if defined(LS_RT_LEAK_TRACKING)
    test_leak_report();
#endif
    measure_alloc();
    measure_io();

    if (ls_rt_test_live_allocations() != 0) {
        failf("live reference-counted allocations at end: %zu",
              ls_rt_test_live_allocations());
    }
    failures = (int)g_failures;
    if (failures == 0) {
        printf("ALL STRESS CHECKS PASSED\n");
    } else {
        printf("STRESS FAILURES: %d\n", failures);
    }
    fflush(stdout);
    ls_runtime_shutdown();
    return failures == 0 ? 0 : 1;
}
