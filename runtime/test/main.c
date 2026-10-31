/* Smoke test for the lsaca C17 runtime.
 *
 * Build (from an ASCII directory, because the repo path has non-ASCII bytes):
 *     gcc -std=c17 -Wall -Wextra -Wpedantic -O1 main.c lsaca_rt.c \
 *         lsaca_rt_string.c lsaca_rt_vec.c lsaca_rt_fmt.c lsaca_rt_map.c \
 *         lsaca_rt_unicode.c -lm -o smoke
 *     smoke                 -- in-process checks (no file IO)
 *     smoke --io <file>     -- in-process checks including file IO
 *     smoke --args a b      -- prints the language-level argument vector
 *     smoke --stdin         -- echoes lines read with ls_read_line
 *     smoke --exit-42       -- tests ls_exit(42)
 *     smoke --panic-<case>  -- panic-path tests, must exit 101
 *     smoke --leak          -- leaks two objects (only reported by debug builds)
 *     smoke --leak-managed  -- leaks one cycle-capable managed allocation
 *     smoke --orc-shutdown  -- leaves an unreachable cycle for shutdown to collect
 *
 * The run_smoke.ps1 script drives all of this and checks exit codes, stdout and
 * stderr. This file is not part of the generated CMake project.
 */

#include "lsaca_rt.h"
#include "lsaca_rt_unicode.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Location used by the panic tests (checked verbatim by run_smoke.ps1). */
#define TEST_FILE "smoke.c"

static int g_checks = 0;
static int g_failures = 0;

#define CHECK(cond)                                                            \
    do {                                                                       \
        g_checks += 1;                                                         \
        if (!(cond)) {                                                         \
            g_failures += 1;                                                   \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond);    \
        }                                                                      \
    } while (0)

static bool bytes_equal(const LsString* s, const char* expected,
                        size_t expected_len) {
    return s != NULL && ls_str_len(s) == expected_len &&
           (expected_len == 0 ||
            memcmp(ls_str_data(s), expected, expected_len) == 0);
}

static void check_str_at(const LsString* actual, const char* expected,
                         const char* expression, int line) {
    size_t expected_len = strlen(expected);
    g_checks += 1;
    if (!bytes_equal(actual, expected, expected_len)) {
        g_failures += 1;
        fprintf(stderr, "FAIL %s:%d: %s\n  expected: \"%s\"\n", __FILE__, line,
                expression, expected);
        if (actual == NULL) {
            fprintf(stderr, "  actual:   NULL\n");
        } else {
            size_t n = ls_str_len(actual);
            fprintf(stderr, "  actual (%zu bytes): \"", n);
            fwrite(ls_str_data(actual), 1, n, stderr);
            fprintf(stderr, "\"\n");
        }
    }
}

#define CHECK_STR(actual, expected)                                            \
    check_str_at((actual), (expected), #actual, __LINE__)

static void check_rendered(LsString* actual, const char* expected, int line) {
    check_str_at(actual, expected, "rendered string", line);
    ls_release(actual);
}

#define CHECK_RENDER(expression, expected)                                     \
    check_rendered((expression), (expected), __LINE__)

static LsString* render_i64(int64_t v) {
    return ls_fmt_i64(ls_format_begin(0), v);
}

static LsString* render_u64(uint64_t v) {
    return ls_fmt_u64(ls_format_begin(0), v);
}

static LsString* render_bool(bool v) {
    return ls_fmt_bool(ls_format_begin(0), v);
}

static LsString* render_f64(double v) {
    return ls_fmt_f64(ls_format_begin(0), v);
}

static LsString* render_f64_debug(double v) {
    return ls_fmt_f64_debug(ls_format_begin(0), v);
}

static LsString* render_f32(float v) {
    return ls_fmt_f32(ls_format_begin(0), v);
}

static LsString* render_f32_debug(float v) {
    return ls_fmt_f32_debug(ls_format_begin(0), v);
}

/* Convenience wrappers that build C-string operands, run the checked operation,
 * and release every temporary. They keep the default suite leak free even when
 * the runtime reports leaked reference-counted allocations. */
static bool eq_cstr(const LsString* a, const char* b) {
    size_t len = strlen(b);
    return a != NULL && ls_str_len(a) == len &&
           (len == 0 || memcmp(ls_str_data(a), b, len) == 0);
}

static int32_t cmp_cstr(const LsString* a, const char* b) {
    LsString* rhs = ls_str_cstr(b);
    int32_t order = ls_str_cmp(a, rhs);
    ls_release(rhs);
    return order;
}

static bool contains_cstr(const LsString* haystack, const char* needle) {
    LsString* n = ls_str_cstr(needle);
    bool found = ls_str_contains(haystack, n);
    ls_release(n);
    return found;
}

static bool starts_with_cstr(const LsString* s, const char* prefix) {
    LsString* p = ls_str_cstr(prefix);
    bool found = ls_str_starts_with(s, p);
    ls_release(p);
    return found;
}

static bool ends_with_cstr(const LsString* s, const char* suffix) {
    LsString* p = ls_str_cstr(suffix);
    bool found = ls_str_ends_with(s, p);
    ls_release(p);
    return found;
}

static int64_t find_cstr(const LsString* haystack, const char* needle) {
    LsString* n = ls_str_cstr(needle);
    int64_t index = ls_str_find(haystack, n);
    ls_release(n);
    return index;
}

static LsString* concat_cstr(const char* a, const char* b) {
    LsString* left = ls_str_cstr(a);
    LsString* right = ls_str_cstr(b);
    LsString* result = ls_str_concat(left, right);
    ls_release(left);
    ls_release(right);
    return result;
}

static LsString* replace_cstr(const char* source, const char* from,
                              const char* to) {
    LsString* s = ls_str_cstr(source);
    LsString* pattern = ls_str_cstr(from);
    LsString* replacement = ls_str_cstr(to);
    LsString* result = ls_str_replace(s, pattern, replacement);
    ls_release(s);
    ls_release(pattern);
    ls_release(replacement);
    return result;
}

static LsString* uppercase_cstr(const char* text) {
    LsString* s = ls_str_cstr(text);
    LsString* result = ls_str_to_uppercase(s);
    ls_release(s);
    return result;
}

static LsString* lowercase_cstr(const char* text) {
    LsString* s = ls_str_cstr(text);
    LsString* result = ls_str_to_lowercase(s);
    ls_release(s);
    return result;
}

static LsString* trim_cstr(const char* text) {
    LsString* s = ls_str_cstr(text);
    LsString* result = ls_str_trim(s);
    ls_release(s);
    return result;
}

static LsString* substring_cstr(const char* text, size_t start, size_t len) {
    LsString* s = ls_str_cstr(text);
    LsString* result = ls_str_substring(s, start, len, TEST_FILE, 0, 0);
    ls_release(s);
    return result;
}

static LsString* repeat_cstr(const char* text, size_t count) {
    LsString* s = ls_str_cstr(text);
    LsString* result = ls_str_repeat(s, count, TEST_FILE, 0, 0);
    ls_release(s);
    return result;
}

/* ====================================================================== */
/* Reference counting and allocation                                      */
/* ====================================================================== */

static void test_alloc(void) {
    void* raw;
    void* alias;

    CHECK(ls_refcount(NULL) == 0);
    CHECK(ls_retain(NULL) == NULL);
    ls_release(NULL); /* must be a no-op */

    raw = ls_alloc(16);
    CHECK(raw != NULL);
    CHECK(ls_refcount(raw) == 1);
    CHECK(((unsigned char*)raw)[0] == 0);
    CHECK(((unsigned char*)raw)[15] == 0);
    CHECK(((uintptr_t)raw % (uintptr_t)_Alignof(max_align_t)) == 0);
    alias = ls_retain(raw);
    CHECK(alias == raw);
    CHECK(ls_refcount(raw) == 2);
    ls_release(raw);
    CHECK(ls_refcount(raw) == 1);
    ls_release(raw);
}

/* ====================================================================== */
/* Reference cycles (ORC collector)                                       */
/* ====================================================================== */

/* Test-harness helpers exported by lsaca_rt.c; deliberately not part of the
 * generated-code ABI. */
extern size_t ls_rt_test_live_allocations(void);
extern size_t ls_rt_test_candidate_threshold(void);
extern size_t ls_rt_test_set_candidate_threshold(size_t threshold);

/* A cycle-capable payload with two managed handles (child, extra) and one
 * unmanaged handle (name). This mirrors the shape of the generated drop and
 * trace functions for an Rc cell; extra lets a garbage cycle reference a live
 * object that is also referenced from outside. */
typedef struct OrcTestNode {
    void* child;
    void* extra;
    LsString* name;
} OrcTestNode;

static int g_orc_drops = 0;

static void orc_test_drop(void* payload) {
    OrcTestNode* node = (OrcTestNode*)payload;
    g_orc_drops += 1;
    ls_release(node->child);
    ls_release(node->extra);
    ls_release(node->name);
}

static void orc_test_trace(const void* payload, LsTraceVisit visit) {
    const OrcTestNode* node = (const OrcTestNode*)payload;
    if (node->child != NULL) {
        visit(node->child);
    }
    if (node->extra != NULL) {
        visit(node->extra);
    }
}

static OrcTestNode* orc_node_new(const char* name) {
    OrcTestNode* node = (OrcTestNode*)ls_alloc_managed(
        sizeof(OrcTestNode), orc_test_drop, orc_test_trace);
    if (name != NULL) {
        node->name = ls_str_cstr(name);
    }
    return node;
}

static void orc_node_set_child(OrcTestNode* node, void* child) {
    if (child != NULL) {
        ls_retain(child);
    }
    ls_release(node->child);
    node->child = child;
}

static void orc_node_set_extra(OrcTestNode* node, void* extra) {
    if (extra != NULL) {
        ls_retain(extra);
    }
    ls_release(node->extra);
    node->extra = extra;
}

/* (a) A two-node cycle with no external references is only reclaimed by an
 * explicit collection; plain reference counting must not free it. */
static void test_orc_cycles(void) {
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    OrcTestNode* a = orc_node_new("a");
    OrcTestNode* b = orc_node_new("b");

    orc_node_set_child(a, b);
    orc_node_set_child(b, a);
    /* Two nodes plus their two names. */
    CHECK(ls_rt_test_live_allocations() == base + 4);
    ls_release(a);
    ls_release(b);
    CHECK(ls_rt_test_live_allocations() == base + 4);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops == drops + 2);
}

/* (b) An acyclic parent/child pair is freed by reference counting alone; the
 * collector must leave the still referenced child alone. */
static void test_orc_acyclic(void) {
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    OrcTestNode* parent = orc_node_new(NULL);
    OrcTestNode* child = orc_node_new(NULL);

    orc_node_set_child(parent, child); /* child count: 1 -> 2 */
    CHECK(ls_rt_test_live_allocations() == base + 2);
    ls_release(parent); /* refcount free; its drop releases the child */
    CHECK(g_orc_drops == drops + 1);
    CHECK(ls_rt_test_live_allocations() == base + 1);
    CHECK(ls_refcount(child) == 1);
    ls_collect_cycles(); /* still referenced: must not be collected */
    CHECK(ls_rt_test_live_allocations() == base + 1);
    CHECK(ls_refcount(child) == 1);
    ls_release(child);
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops == drops + 2);
}

/* (c) A cycle that is still externally referenced through one node survives a
 * collection; releasing that last handle makes it garbage. */
static void test_orc_referenced_cycle(void) {
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    OrcTestNode* a = orc_node_new("x");
    OrcTestNode* b = orc_node_new("y");

    orc_node_set_child(a, b);
    orc_node_set_child(b, a);
    CHECK(ls_refcount(a) == 2);
    ls_release(b); /* a still keeps the cycle externally reachable */
    CHECK(ls_rt_test_live_allocations() == base + 4);
    ls_collect_cycles(); /* reachable from outside: the cycle must survive */
    CHECK(ls_rt_test_live_allocations() == base + 4);
    CHECK(ls_refcount(a) == 2);
    CHECK(ls_refcount(b) == 1);
    CHECK(g_orc_drops == drops);
    ls_release(a);
    CHECK(ls_rt_test_live_allocations() == base + 4);
    ls_collect_cycles(); /* now the whole cycle is garbage */
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops == drops + 2);
}

/* (d) Crossing the candidate threshold triggers collection automatically,
 * without any explicit ls_collect_cycles() call. */
static void test_orc_automatic_collection(void) {
    size_t previous_threshold = ls_rt_test_set_candidate_threshold(8);
    const size_t cycles = 40;
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    size_t i;
    size_t freed;

    for (i = 0; i < cycles; i += 1) {
        OrcTestNode* a = orc_node_new(NULL);
        OrcTestNode* b = orc_node_new(NULL);
        orc_node_set_child(a, b);
        orc_node_set_child(b, a);
        ls_release(a);
        ls_release(b);
    }
    /* The candidate threshold must already have collected some garbage
     * cycles: no explicit collection was requested in the loop. */
    freed = (size_t)(g_orc_drops - drops);
    CHECK(freed > 0);
    CHECK(ls_rt_test_live_allocations() == base + 2 * cycles - freed);
    CHECK(ls_rt_test_live_allocations() < base + 2 * cycles);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops - drops == (int)(2 * cycles));
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
}

/* (e) A garbage cycle that also references a live managed object: the trial
 * deletion already accounts for the cycle's outgoing edges, so freeing the
 * cycle's drops must not decrement the live object's count a second time. The
 * live object must survive with only its external reference. */
static void test_orc_live_edge(void) {
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    OrcTestNode* a = orc_node_new("la");
    OrcTestNode* b = orc_node_new("lb");
    OrcTestNode* live = orc_node_new("live");

    orc_node_set_child(a, b);
    orc_node_set_child(b, a);
    orc_node_set_extra(b, live); /* live: external handle + edge from b */
    CHECK(ls_refcount(live) == 2);
    CHECK(ls_rt_test_live_allocations() == base + 6);
    ls_release(a);
    ls_release(b);
    CHECK(ls_rt_test_live_allocations() == base + 6);
    ls_collect_cycles(); /* frees a and b, keeps the live node */
    CHECK(ls_rt_test_live_allocations() == base + 2);
    CHECK(ls_refcount(live) == 1);
    CHECK(g_orc_drops == drops + 2);
    ls_release(live);
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops == drops + 3);
}

/* ---------------------------------------------------------------------- */
/* Collector stress tests                                                 */
/* ---------------------------------------------------------------------- */

/* Deterministic splitmix64 PRNG shared by the ORC and hash-map stress tests.
 * Stress runs must reproduce bit for bit, so no time seed and no library RNG
 * are used. */
#define STRESS_RNG_SEED ((uint64_t)0x9E3779B97F4A7C15ULL)

static uint64_t g_stress_rng_state = STRESS_RNG_SEED;

static uint64_t stress_rng_next(void) {
    uint64_t z;

    g_stress_rng_state += (uint64_t)0x9E3779B97F4A7C15ULL;
    z = g_stress_rng_state;
    z = (z ^ (z >> 30)) * (uint64_t)0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * (uint64_t)0x94D049BB133111EBULL;
    return z ^ (z >> 31);
}

static void stress_rng_reset(void) {
    g_stress_rng_state = STRESS_RNG_SEED;
}

/* Stress sizes are compile-time constants so that the reported check count
 * stays meaningful: every stress test aggregates its per-operation
 * observations into a handful of summary CHECKs instead of one check per
 * allocation or hash-map operation. */
#define ORC_STRESS_CYCLES ((size_t)5000)
#define ORC_STRESS_RING_NODES ((size_t)2000)
#define ORC_STRESS_CHURN_OBJECTS ((size_t)100000)
#define ORC_STRESS_CHURN_CYCLE_INTERVAL ((size_t)500)
#define ORC_STRESS_CHURN_COLLECT_INTERVAL ((size_t)10000)
#define ORC_STRESS_CHURN_CYCLES \
    ((ORC_STRESS_CHURN_OBJECTS / ORC_STRESS_CHURN_CYCLE_INTERVAL) - 1)

/* Shuffle a node array with Fisher-Yates and the deterministic PRNG. */
static void stress_shuffle_nodes(OrcTestNode** items, size_t count) {
    size_t i;

    for (i = count; i > 1; i -= 1) {
        size_t j = (size_t)(stress_rng_next() % (uint64_t)i);
        OrcTestNode* tmp = items[i - 1];
        items[i - 1] = items[j];
        items[j] = tmp;
    }
}

/* (f) 5000 independent two-node cycles: reference counting alone must never
 * reclaim them, and a single collection must return the live count to the
 * baseline. Automatic collection is disabled while the cycles are built. */
static void test_orc_stress_many_cycles(void) {
    size_t previous_threshold = ls_rt_test_set_candidate_threshold(SIZE_MAX);
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    size_t i;

    for (i = 0; i < ORC_STRESS_CYCLES; i += 1) {
        OrcTestNode* a = orc_node_new(NULL);
        OrcTestNode* b = orc_node_new(NULL);
        orc_node_set_child(a, b);
        orc_node_set_child(b, a);
        ls_release(a);
        ls_release(b);
    }
    /* No candidate threshold was crossed: the cycles are still alive and
     * prove that plain reference counting does not reclaim cycles. */
    CHECK(ls_rt_test_live_allocations() == base + 2 * ORC_STRESS_CYCLES);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops - drops == (int)(2 * ORC_STRESS_CYCLES));
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
}

/* (g) One large cycle of 2000 nodes. Every node's only managed field points
 * to the next node, so the collector's intrusive mark/scan/collect visitors
 * recurse around the ring instead of over many small components. */
static OrcTestNode* s_orc_ring[ORC_STRESS_RING_NODES];

static void test_orc_stress_large_cycle(void) {
    size_t previous_threshold = ls_rt_test_set_candidate_threshold(SIZE_MAX);
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    size_t i;

    for (i = 0; i < ORC_STRESS_RING_NODES; i += 1) {
        s_orc_ring[i] = orc_node_new(NULL);
    }
    for (i = 0; i < ORC_STRESS_RING_NODES; i += 1) {
        orc_node_set_child(s_orc_ring[i],
                           s_orc_ring[(i + 1) % ORC_STRESS_RING_NODES]);
    }
    CHECK(ls_rt_test_live_allocations() == base + ORC_STRESS_RING_NODES);
    for (i = 0; i < ORC_STRESS_RING_NODES; i += 1) {
        ls_release(s_orc_ring[i]);
    }
    /* The ring is garbage now, but every node still has an internal edge. */
    CHECK(ls_rt_test_live_allocations() == base + ORC_STRESS_RING_NODES);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops - drops == (int)ORC_STRESS_RING_NODES);
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
}

/* (h) Edges between garbage and live objects in both directions. */
static void test_orc_stress_live_edges(void) {
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    OrcTestNode* garbage_a = orc_node_new("garbage-a");
    OrcTestNode* garbage_b = orc_node_new("garbage-b");
    OrcTestNode* live_root = orc_node_new("live-root");
    OrcTestNode* cycle_x;
    OrcTestNode* cycle_y;
    OrcTestNode* live_outer;

    /* (1) A garbage cycle points at a live root. The collection must drop the
     * cycle and leave the live root with only its external reference. */
    orc_node_set_child(garbage_a, garbage_b);
    orc_node_set_child(garbage_b, garbage_a);
    orc_node_set_extra(garbage_b, live_root);
    CHECK(ls_refcount(live_root) == 2);
    ls_release(garbage_a);
    ls_release(garbage_b);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base + 2); /* root + its name */
    CHECK(ls_refcount(live_root) == 1);
    CHECK_STR(live_root->name, "live-root");
    CHECK(g_orc_drops - drops == 2);

    /* (2) A live root points into an otherwise garbage cycle. For the trial
     * deletion that edge is external, so the whole reachable component must
     * survive the collection. */
    cycle_x = orc_node_new("cycle-x");
    cycle_y = orc_node_new("cycle-y");
    live_outer = orc_node_new("live-outer");
    orc_node_set_child(cycle_x, cycle_y);
    orc_node_set_child(cycle_y, cycle_x);
    orc_node_set_child(live_outer, cycle_x);
    CHECK(ls_refcount(cycle_x) == 3);
    CHECK(ls_refcount(cycle_y) == 2);
    CHECK(ls_refcount(live_outer) == 1);
    ls_release(cycle_x);
    ls_release(cycle_y);
    CHECK(ls_rt_test_live_allocations() == base + 2 + 6);
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base + 8);
    CHECK(ls_refcount(cycle_x) == 2);
    CHECK(ls_refcount(cycle_y) == 1);
    CHECK(ls_refcount(live_outer) == 1);
    CHECK_STR(cycle_x->name, "cycle-x");
    CHECK_STR(live_outer->name, "live-outer");
    CHECK(g_orc_drops - drops == 2);
    ls_release(live_outer); /* frees outer, releases its edge to cycle_x */
    CHECK(ls_rt_test_live_allocations() == base + 6);
    CHECK(g_orc_drops - drops == 3);
    ls_collect_cycles(); /* the cycle is garbage once the root is gone */
    CHECK(ls_rt_test_live_allocations() == base + 2);
    CHECK(g_orc_drops - drops == 5);
    ls_release(live_root);
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops - drops == 6);
}

/* (i) Random churn: 100k managed allocations released in a PRNG-driven order,
 * a two-node cycle created every 500 allocations, and periodic collections.
 * Deterministic by construction; the whole test stays well under a second. */
static OrcTestNode* s_orc_churn[ORC_STRESS_CHURN_OBJECTS];

static void test_orc_stress_random_churn(void) {
    size_t previous_threshold = ls_rt_test_set_candidate_threshold(SIZE_MAX);
    size_t base = ls_rt_test_live_allocations();
    int drops = g_orc_drops;
    size_t i;
    size_t cycles = 0;
    size_t pending_cycles = 0;

    stress_rng_reset();
    for (i = 0; i < ORC_STRESS_CHURN_OBJECTS; i += 1) {
        s_orc_churn[i] = orc_node_new(NULL);
        if (i > 0 && (i % ORC_STRESS_CHURN_CYCLE_INTERVAL) == 0) {
            OrcTestNode* a = orc_node_new(NULL);
            OrcTestNode* b = orc_node_new(NULL);
            orc_node_set_child(a, b);
            orc_node_set_child(b, a);
            ls_release(a);
            ls_release(b);
            cycles += 1;
            pending_cycles += 1;
        }
        if ((i % ORC_STRESS_CHURN_COLLECT_INTERVAL) ==
            ORC_STRESS_CHURN_COLLECT_INTERVAL - 1) {
            ls_collect_cycles();
            pending_cycles = 0;
        }
    }
    CHECK(cycles == ORC_STRESS_CHURN_CYCLES);
    CHECK(ls_rt_test_live_allocations() ==
          base + ORC_STRESS_CHURN_OBJECTS + 2 * pending_cycles);
    stress_shuffle_nodes(s_orc_churn, ORC_STRESS_CHURN_OBJECTS);
    for (i = 0; i < ORC_STRESS_CHURN_OBJECTS; i += 1) {
        ls_release(s_orc_churn[i]);
        if ((i % ORC_STRESS_CHURN_COLLECT_INTERVAL) ==
            ORC_STRESS_CHURN_COLLECT_INTERVAL - 1) {
            ls_collect_cycles();
        }
    }
    ls_collect_cycles();
    CHECK(ls_rt_test_live_allocations() == base);
    CHECK(g_orc_drops - drops ==
          (int)(ORC_STRESS_CHURN_OBJECTS + 2 * cycles));
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
}

/* ====================================================================== */
/* Strings: construction, observation, cloning                            */
/* ====================================================================== */

static void test_strings(void) {
    LsString* empty = ls_str_new();
    LsString* hello = ls_str_cstr("hello");
    LsString* copy;
    LsString* clone;

    CHECK_STR(empty, "");
    CHECK(ls_str_len(empty) == 0);
    CHECK(ls_str_is_empty(empty));
    CHECK(ls_str_data(empty)[0] == '\0');

    CHECK_STR(hello, "hello");
    CHECK(ls_str_len(hello) == 5);
    CHECK(!ls_str_is_empty(hello));
    CHECK(ls_str_data(hello)[5] == '\0');

    copy = (LsString*)ls_retain((void*)hello);
    CHECK(copy == hello);
    CHECK(ls_refcount(hello) == 2);
    ls_release(copy);
    CHECK(ls_refcount(hello) == 1);

    clone = ls_str_clone(hello);
    CHECK(clone != hello);
    CHECK(ls_refcount(clone) == 1);
    CHECK_STR(clone, "hello");
    CHECK(ls_str_eq(hello, clone));
    ls_release(clone);

    CHECK(eq_cstr(hello, "hello"));
    CHECK(!eq_cstr(hello, "Hello"));

    {
        const char raw[] = {'a', '\0', 'b'};
        LsString* embedded = ls_str_from_parts(raw, 3);
        CHECK(ls_str_len(embedded) == 3);
        CHECK(memcmp(ls_str_data(embedded), raw, 3) == 0);
        CHECK(ls_str_data(embedded)[3] == '\0');
        ls_release(embedded);
    }
    {
        LsString* reserved = ls_str_with_capacity(64);
        CHECK(ls_str_len(reserved) == 0);
        ls_release(reserved);
    }

    CHECK(cmp_cstr(hello, "hello") == 0);
    CHECK(cmp_cstr(hello, "hellp") < 0);
    CHECK(cmp_cstr(hello, "hell") > 0);

    ls_release(empty);
    ls_release(hello);
}

/* ====================================================================== */
/* Copy on write and in-place mutation                                    */
/* ====================================================================== */

static void test_cow(void) {
    LsString* hello = ls_str_cstr("hello");
    LsString* alias = (LsString*)ls_retain((void*)hello);
    LsString* suffix = ls_str_cstr(", world");

    CHECK(ls_refcount(hello) == 2);
    hello = ls_str_push_str(hello, suffix);
    CHECK(ls_refcount(hello) == 1);
    CHECK(ls_refcount(alias) == 1);
    CHECK_STR(alias, "hello");
    CHECK_STR(hello, "hello, world");
    ls_release(alias);
    ls_release(hello);
    ls_release(suffix);

    /* A unique string with spare capacity mutates in place. */
    {
        LsString* s = ls_str_with_capacity(32);
        const char* before;
        s = ls_str_push_cstr(s, "abc");
        before = ls_str_data(s);
        s = ls_str_push_char(s, 'd');
        CHECK(ls_str_data(s) == before);
        CHECK_STR(s, "abcd");

        s = ls_str_clear(s);
        CHECK_STR(s, "");
        CHECK(ls_str_len(s) == 0);
        CHECK(ls_str_data(s)[0] == '\0');
        CHECK(ls_str_data(s) == before); /* clear keeps the buffer */

        s = ls_str_push_cstr(s, "xyz");
        CHECK_STR(s, "xyz");

        /* Self append doubles the content. */
        s = ls_str_push_str(s, s);
        CHECK_STR(s, "xyzxyz");
        ls_release(s);
    }

    /* Appending a string to itself works even while it is shared. */
    {
        LsString* s = ls_str_cstr("ab");
        LsString* shared = (LsString*)ls_retain((void*)s);
        s = ls_str_push_str(s, s);
        CHECK_STR(s, "abab");
        CHECK_STR(shared, "ab");
        ls_release(shared);
        ls_release(s);
    }

    /* reserve makes room without changing the contents. */
    {
        LsString* s = ls_str_cstr("abc");
        s = ls_str_reserve(s, 100);
        CHECK_STR(s, "abc");
        s = ls_str_push_char(s, '!');
        CHECK_STR(s, "abc!");
        ls_release(s);
    }
}

/* ====================================================================== */
/* Search, replacement and transformations                                */
/* ====================================================================== */

static void test_search_transform(void) {
    LsString* hay = ls_str_cstr("banana");
    LsString* na = ls_str_cstr("na");
    LsString* missing = ls_str_cstr("zzz");
    LsString* result;

    CHECK(ls_str_contains(hay, na));
    CHECK(ls_str_find(hay, na) == 2);
    CHECK(!ls_str_contains(hay, missing));
    CHECK(ls_str_find(hay, missing) == -1);
    CHECK(find_cstr(hay, "banana") == 0);
    CHECK(starts_with_cstr(hay, "ban"));
    CHECK(!starts_with_cstr(hay, "nan"));
    CHECK(ends_with_cstr(hay, "ana"));
    CHECK(!ends_with_cstr(hay, "ban"));
    CHECK(starts_with_cstr(hay, ""));
    CHECK(ends_with_cstr(hay, ""));
    CHECK(find_cstr(hay, "") == 0);
    {
        LsString* ab = ls_str_cstr("ab");
        CHECK(find_cstr(ab, "abc") == -1);
        ls_release(ab);
    }

    result = concat_cstr("foo", "bar");
    CHECK_STR(result, "foobar");
    ls_release(result);

    result = replace_cstr("aXbXc", "X", "--");
    CHECK_STR(result, "a--b--c");
    ls_release(result);

    result = replace_cstr("a--b--c", "--", "");
    CHECK_STR(result, "abc");
    ls_release(result);

    /* Rust: "ab".replace("", "-") == "-a-b-". */
    result = replace_cstr("ab", "", "-");
    CHECK_STR(result, "-a-b-");
    ls_release(result);

    /* Nothing to replace: a copy with unchanged contents. */
    result = replace_cstr("abc", "Z", "Y");
    CHECK_STR(result, "abc");
    ls_release(result);

    result = uppercase_cstr("aBc1-Z");
    CHECK_STR(result, "ABC1-Z");
    ls_release(result);

    result = lowercase_cstr("AbC1-Z");
    CHECK_STR(result, "abc1-z");
    ls_release(result);

    result = trim_cstr(" \t\r\n hi there \r\n ");
    CHECK_STR(result, "hi there");
    ls_release(result);

    result = trim_cstr("   ");
    CHECK_STR(result, "");
    ls_release(result);

    result = substring_cstr("hello", 1, 3);
    CHECK_STR(result, "ell");
    ls_release(result);

    result = substring_cstr("hello", 5, 0);
    CHECK_STR(result, "");
    ls_release(result);

    result = repeat_cstr("ab", 3);
    CHECK_STR(result, "ababab");
    ls_release(result);

    result = repeat_cstr("ab", 0);
    CHECK_STR(result, "");
    ls_release(result);

    ls_release(hay);
    ls_release(na);
    ls_release(missing);
}

/* Splitting returns a vector of freshly allocated strings; the vector owns one
 * reference to each of them. */
static void test_split(void) {
    LsString* source = ls_str_cstr("a,b,,c");
    LsString* separator = ls_str_cstr(",");
    LsVec_String parts;

    parts = ls_str_split(source, separator);
    CHECK(parts.len == 4);
    if (parts.len == 4) {
        CHECK_STR(parts.data[0], "a");
        CHECK_STR(parts.data[1], "b");
        CHECK_STR(parts.data[2], "");
        CHECK_STR(parts.data[3], "c");
    }
    ls_vec_string_free(&parts);

    /* Empty separator splits into UTF-8 scalar values, with empty ends. */
    {
        LsString* text = ls_str_cstr("ab");
        LsString* empty_sep = ls_str_cstr("");
        LsVec_String chars = ls_str_split(text, empty_sep);
        CHECK(chars.len == 4);
        if (chars.len == 4) {
            CHECK_STR(chars.data[0], "");
            CHECK_STR(chars.data[1], "a");
            CHECK_STR(chars.data[2], "b");
            CHECK_STR(chars.data[3], "");
        }
        ls_vec_string_free(&chars);
        ls_release(text);
        ls_release(empty_sep);
    }
    {
        LsString* text = ls_str_cstr("\xC3\xA9"); /* U+00E9, two UTF-8 bytes */
        LsString* empty_sep = ls_str_cstr("");
        LsVec_String pieces = ls_str_split(text, empty_sep);
        CHECK(pieces.len == 3);
        if (pieces.len == 3) {
            CHECK_STR(pieces.data[0], "");
            CHECK_STR(pieces.data[1], "\xC3\xA9");
            CHECK_STR(pieces.data[2], "");
        }
        ls_vec_string_free(&pieces);
        ls_release(text);
        ls_release(empty_sep);
    }
    {
        LsString* text = ls_str_cstr("");
        LsString* sep = ls_str_cstr(",");
        LsVec_String pieces = ls_str_split(text, sep);
        CHECK(pieces.len == 1);
        if (pieces.len == 1) {
            CHECK_STR(pieces.data[0], "");
        }
        ls_vec_string_free(&pieces);
        ls_release(text);
        ls_release(sep);
    }
    {
        LsString* text = ls_str_cstr("");
        LsString* empty_sep = ls_str_cstr("");
        LsVec_String pieces = ls_str_split(text, empty_sep);
        CHECK(pieces.len == 2);
        ls_vec_string_free(&pieces);
        ls_release(text);
        ls_release(empty_sep);
    }
    ls_release(source);
    ls_release(separator);
}

/* ====================================================================== */
/* Parsing                                                                */
/* ====================================================================== */

static void check_parse_i64(const char* text, bool expected_ok,
                            int64_t expected_value) {
    LsString* s = ls_str_cstr(text);
    int64_t value = 0;
    bool ok = ls_str_parse_i64(s, &value);
    g_checks += 1;
    if (ok != expected_ok || (ok && value != expected_value)) {
        g_failures += 1;
        fprintf(stderr, "FAIL i64 parse \"%s\": ok=%d value=%lld\n", text, ok,
                (long long)value);
    }
    ls_release(s);
}

static void check_parse_f64(const char* text, bool expected_ok,
                            double expected_value) {
    LsString* s = ls_str_cstr(text);
    double value = 0.0;
    bool ok = ls_str_parse_f64(s, &value);
    bool same;
    if (isnan(expected_value)) {
        same = ok && isnan(value);
    } else if (isinf(expected_value)) {
        same = ok && isinf(value) && ((value > 0) == (expected_value > 0));
    } else {
        same = ok && value == expected_value;
    }
    g_checks += 1;
    if (ok != expected_ok || (ok && !same)) {
        g_failures += 1;
        fprintf(stderr, "FAIL f64 parse \"%s\": ok=%d value=%g\n", text, ok,
                value);
    }
    ls_release(s);
}

static void test_parse(void) {
    check_parse_i64("0", true, 0);
    check_parse_i64("-0", true, 0);
    check_parse_i64("+5", true, 5);
    check_parse_i64("-42", true, -42);
    check_parse_i64("9223372036854775807", true, INT64_MAX);
    check_parse_i64("-9223372036854775808", true, INT64_MIN);
    check_parse_i64("", false, 0);
    check_parse_i64("+", false, 0);
    check_parse_i64("-", false, 0);
    check_parse_i64(" 5", false, 0);
    check_parse_i64("5 ", false, 0);
    check_parse_i64("0x10", false, 0);
    check_parse_i64("1.0", false, 0);
    check_parse_i64("9223372036854775808", false, 0);
    check_parse_i64("-9223372036854775809", false, 0);
    check_parse_i64("abc", false, 0);
    check_parse_i64("1_0", false, 0);

    check_parse_f64("1.", true, 1.0);
    check_parse_f64(".5", true, 0.5);
    check_parse_f64("+2.5", true, 2.5);
    check_parse_f64("-1e3", true, -1000.0);
    check_parse_f64("1E5", true, 100000.0);
    check_parse_f64("0.1", true, 0.1);
    check_parse_f64("1e999", true, INFINITY);
    check_parse_f64("-1e999", true, -INFINITY);
    check_parse_f64("inf", true, INFINITY);
    check_parse_f64("-inf", true, -INFINITY);
    check_parse_f64("infinity", true, INFINITY);
    check_parse_f64("NaN", true, NAN);
    check_parse_f64("", false, 0.0);
    check_parse_f64(".", false, 0.0);
    check_parse_f64(" 1", false, 0.0);
    check_parse_f64("1 ", false, 0.0);
    check_parse_f64("0x10", false, 0.0);
    check_parse_f64("1.2.3", false, 0.0);
    check_parse_f64("e5", false, 0.0);
    check_parse_f64("1e", false, 0.0);
    check_parse_f64("1e+", false, 0.0);
    check_parse_f64("--1", false, 0.0);
    check_parse_f64("nanx", false, 0.0);
}

/* ====================================================================== */
/* LsVec_String                                                           */
/* ====================================================================== */

static void push_new(LsVec_String* v, const char* text) {
    LsString* s = ls_str_cstr(text);
    ls_vec_string_push(v, s);
    ls_release(s);
}

static void test_vec_string(void) {
    LsVec_String v = ls_vec_string_new();
    LsString* first;
    LsString* got = NULL;

    CHECK(v.len == 0);
    CHECK(v.cap == 0);
    CHECK(v.data == NULL);

    first = ls_str_cstr("first");
    ls_vec_string_push(&v, first);
    CHECK(ls_refcount(first) == 2);
    ls_release(first);
    CHECK(v.len == 1);
    CHECK(ls_refcount(v.data[0]) == 1);
    CHECK_STR(v.data[0], "first");

    push_new(&v, "second");
    push_new(&v, "third");
    CHECK(v.len == 3);

    {
        LsString* inserted = ls_str_cstr("inserted");
        CHECK(ls_refcount(inserted) == 1);
        ls_vec_string_insert(&v, 1, inserted);
        CHECK(ls_refcount(inserted) == 2); /* the vector owns its own reference */
        ls_release(inserted);
    }
    CHECK(v.len == 4);
    CHECK_STR(v.data[0], "first");
    CHECK_STR(v.data[1], "inserted");
    CHECK_STR(v.data[2], "second");
    CHECK_STR(v.data[3], "third");

    CHECK(ls_vec_string_get(&v, 1, &got));
    CHECK(got != NULL);
    CHECK_STR(got, "inserted");
    CHECK(ls_refcount(got) == 2); /* vector + caller */
    ls_release(got);
    got = NULL;
    CHECK(!ls_vec_string_get(&v, 99, &got));

    {
        LsString* removed = ls_vec_string_remove(&v, 1);
        CHECK(removed != NULL);
        CHECK_STR(removed, "inserted");
        CHECK(ls_refcount(removed) == 1);
        ls_release(removed);
    }
    CHECK(v.len == 3); /* [first, second, third] */
    CHECK_STR(v.data[1], "second");

    {
        LsString* popped = NULL;
        CHECK(ls_vec_string_pop(&v, &popped));
        CHECK(popped != NULL);
        CHECK_STR(popped, "third");
        ls_release(popped);
        popped = NULL;
        CHECK(ls_vec_string_pop(&v, &popped));
        CHECK_STR(popped, "second");
        ls_release(popped);
        popped = NULL;
        CHECK(ls_vec_string_pop(&v, &popped));
        CHECK_STR(popped, "first");
        ls_release(popped);
        popped = NULL;
        CHECK(!ls_vec_string_pop(&v, &popped)); /* empty */
    }
    CHECK(v.len == 0);

    push_new(&v, "first");
    push_new(&v, "second");
    push_new(&v, "third");
    ls_vec_string_reverse(&v);
    CHECK_STR(v.data[0], "third");
    CHECK_STR(v.data[1], "second");
    CHECK_STR(v.data[2], "first");

    {
        LsVec_String other = ls_vec_string_new();
        push_new(&other, "x");
        push_new(&other, "y");
        ls_vec_string_extend(&v, &other);
        CHECK(v.len == 5);
        CHECK_STR(v.data[3], "x");
        CHECK_STR(v.data[4], "y");
        CHECK(ls_refcount(other.data[0]) == 2);
        ls_vec_string_free(&other);
        CHECK(ls_refcount(v.data[3]) == 1);
    }

    /* Extending a vector with itself duplicates every element. */
    {
        LsVec_String self = ls_vec_string_new();
        push_new(&self, "a");
        push_new(&self, "b");
        ls_vec_string_extend(&self, &self);
        CHECK(self.len == 4);
        CHECK_STR(self.data[0], "a");
        CHECK_STR(self.data[1], "b");
        CHECK_STR(self.data[2], "a");
        CHECK_STR(self.data[3], "b");
        CHECK(ls_refcount(self.data[0]) == 2);
        CHECK(ls_refcount(self.data[1]) == 2);
        ls_vec_string_clear(&self);
        CHECK(self.len == 0);
        ls_vec_string_free(&self);
    }

    {
        LsVec_String with_cap = ls_vec_string_with_capacity(16);
        CHECK(with_cap.len == 0);
        CHECK(with_cap.cap >= 16);
        ls_vec_string_free(&with_cap);
    }

    ls_vec_string_clear(&v);
    CHECK(v.len == 0);
    ls_vec_string_free(&v);
    CHECK(v.data == NULL);
    CHECK(v.cap == 0);
}

/* ====================================================================== */
/* Generic vector primitives                                              */
/* ====================================================================== */

static void test_vec_primitives(void) {
    int* data = (int*)ls_vec_alloc(2, sizeof(int));
    size_t cap = 2;
    size_t len = 0;
    size_t i;

    CHECK(data != NULL);
    CHECK(data[0] == 0);
    CHECK(data[1] == 0);
    CHECK(ls_vec_alloc(0, sizeof(int)) == NULL);

    for (i = 0; i < 5; i += 1) {
        ls_vec_grow((void**)&data, &cap, len, sizeof(int));
        data[len] = (int)i;
        len += 1;
    }
    CHECK(len == 5);
    CHECK(cap >= 5);
    for (i = 0; i < 5; i += 1) {
        CHECK(data[i] == (int)i);
    }

    {
        int value = 99;
        ls_vec_insert_bytes((void**)&data, &len, &cap, 2, &value, sizeof(int));
        CHECK(len == 6);
        CHECK(data[2] == 99);
        CHECK(data[3] == 2);
        CHECK(data[5] == 4);
    }
    {
        int removed = -1;
        ls_vec_remove_bytes(data, &len, 2, &removed, sizeof(int));
        CHECK(len == 5);
        CHECK(removed == 99);
        CHECK(data[2] == 2);
    }
    {
        int extra[2];
        extra[0] = 7;
        extra[1] = 8;
        ls_vec_extend_bytes((void**)&data, &len, &cap, extra, 2, sizeof(int));
        CHECK(len == 7);
        CHECK(data[5] == 7);
        CHECK(data[6] == 8);
    }
    ls_vec_reverse_bytes(data, len, sizeof(int));
    CHECK(data[0] == 8);
    CHECK(data[6] == 0);

    {
        int* copy = (int*)ls_vec_clone_bytes(data, len, sizeof(int));
        CHECK(copy != NULL);
        CHECK(copy != data);
        CHECK(memcmp(copy, data, len * sizeof(int)) == 0);
        ls_vec_free_bytes(copy);
    }
    {
        void* reserved = NULL;
        size_t reserved_cap = 0;
        ls_vec_reserve(&reserved, &reserved_cap, 0, 10, sizeof(int));
        CHECK(reserved != NULL);
        CHECK(reserved_cap >= 10);
        ls_vec_free_bytes(reserved);
    }
    ls_vec_free_bytes(data);
}

/* ====================================================================== */
/* Formatting                                                             */
/* ====================================================================== */

static void check_expanded_power_of_ten(LsString* s, size_t exponent, int line) {
    size_t i;
    size_t expected_len = exponent + 1;
    bool ok = s != NULL && ls_str_len(s) == expected_len &&
              ls_str_data(s)[0] == '1';
    if (ok) {
        for (i = 1; i < expected_len; i += 1) {
            if (ls_str_data(s)[i] != '0') {
                ok = false;
                break;
            }
        }
    }
    g_checks += 1;
    if (!ok) {
        g_failures += 1;
        fprintf(stderr, "FAIL %s:%d: expanded 1e%zu\n", __FILE__, line,
                exponent);
    }
    ls_release(s);
}

static void test_formatting(void) {
    CHECK_RENDER(render_i64(0), "0");
    CHECK_RENDER(render_i64(-42), "-42");
    CHECK_RENDER(render_i64(INT64_MAX), "9223372036854775807");
    CHECK_RENDER(render_i64(INT64_MIN), "-9223372036854775808");
    CHECK_RENDER(render_u64(UINT64_MAX), "18446744073709551615");
    CHECK_RENDER(render_u64(0), "0");
    CHECK_RENDER(render_bool(true), "true");
    CHECK_RENDER(render_bool(false), "false");

    /* Builder sequence: every formatter consumes the accumulator handle. */
    {
        LsString* piece = ls_str_cstr("xyz");
        LsString* acc = ls_format_begin(4);
        acc = ls_format_push_cstr(acc, "n=");
        acc = ls_fmt_i64(acc, -42);
        acc = ls_format_push_char(acc, ' ');
        acc = ls_fmt_f64(acc, 1.5);
        acc = ls_format_push_str(acc, piece);
        acc = ls_fmt_bool(acc, true);
        CHECK_STR(acc, "n=-42 1.5xyztrue");
        ls_release(acc);
        ls_release(piece);
    }
    {
        /* A shared accumulator detaches instead of corrupting the alias. */
        LsString* acc = ls_str_cstr("base");
        LsString* alias = (LsString*)ls_retain((void*)acc);
        acc = ls_fmt_i64(acc, 7);
        CHECK_STR(acc, "base7");
        CHECK_STR(alias, "base");
        ls_release(alias);
        ls_release(acc);
    }

    /* f64 Display (no exponent, shortest round-trip). */
    CHECK_RENDER(render_f64(0.0), "0");
    CHECK_RENDER(render_f64(-0.0), "-0");
    CHECK_RENDER(render_f64(1.0), "1");
    CHECK_RENDER(render_f64(-1.0), "-1");
    CHECK_RENDER(render_f64(0.5), "0.5");
    CHECK_RENDER(render_f64(-2.5), "-2.5");
    CHECK_RENDER(render_f64(100.0), "100");
    CHECK_RENDER(render_f64(123456789.0), "123456789");
    CHECK_RENDER(render_f64(0.1), "0.1");
    CHECK_RENDER(render_f64(1.2345678901234567), "1.2345678901234567");
    CHECK_RENDER(render_f64(1e15), "1000000000000000");
    CHECK_RENDER(render_f64(1e16), "10000000000000000");
    CHECK_RENDER(render_f64(1e-4), "0.0001");
    CHECK_RENDER(render_f64(1e-5), "0.00001");
    CHECK_RENDER(render_f64(INFINITY), "inf");
    CHECK_RENDER(render_f64(-INFINITY), "-inf");
    CHECK_RENDER(render_f64(NAN), "NaN");
    check_expanded_power_of_ten(render_f64(1e300), 300, __LINE__);
    check_expanded_power_of_ten(render_f64(1e100), 100, __LINE__);

    /* f64 Debug: integral ".0", exponent outside 1e-4 .. 1e16. */
    CHECK_RENDER(render_f64_debug(0.0), "0.0");
    CHECK_RENDER(render_f64_debug(-0.0), "-0.0");
    CHECK_RENDER(render_f64_debug(1.0), "1.0");
    CHECK_RENDER(render_f64_debug(1.5), "1.5");
    CHECK_RENDER(render_f64_debug(100.0), "100.0");
    CHECK_RENDER(render_f64_debug(123456789.0), "123456789.0");
    CHECK_RENDER(render_f64_debug(1e15), "1000000000000000.0");
    CHECK_RENDER(render_f64_debug(1e16), "1e16");
    CHECK_RENDER(render_f64_debug(1e-4), "0.0001");
    CHECK_RENDER(render_f64_debug(1e-5), "1e-5");
    CHECK_RENDER(render_f64_debug(1e300), "1e300");
    CHECK_RENDER(render_f64_debug(INFINITY), "inf");
    CHECK_RENDER(render_f64_debug(-INFINITY), "-inf");
    CHECK_RENDER(render_f64_debug(NAN), "NaN");

    /* f32 is formatted from its own shortest digits. */
    CHECK_RENDER(render_f32(0.0f), "0");
    CHECK_RENDER(render_f32(-0.0f), "-0");
    CHECK_RENDER(render_f32(1.0f), "1");
    CHECK_RENDER(render_f32(0.1f), "0.1");
    CHECK_RENDER(render_f32(1.5f), "1.5");
    CHECK_RENDER(render_f32(1.0f / 3.0f), "0.33333334");
    CHECK_RENDER(render_f32(16777216.0f), "16777216");
    CHECK_RENDER(render_f32(1e16f), "10000000000000000");
    CHECK_RENDER(render_f32(1e-5f), "0.00001");
    CHECK_RENDER(render_f32(INFINITY), "inf");
    CHECK_RENDER(render_f32(-INFINITY), "-inf");
    CHECK_RENDER(render_f32(NAN), "NaN");
    {
        LsString* max_display = render_f32(3.4028235e38f);
        CHECK(ls_str_len(max_display) == 39);
        CHECK(memcmp(ls_str_data(max_display),
                     "34028235", 8) == 0);
        {
            size_t i;
            bool zeros = true;
            for (i = 8; i < ls_str_len(max_display); i += 1) {
                if (ls_str_data(max_display)[i] != '0') {
                    zeros = false;
                }
            }
            CHECK(zeros);
        }
        ls_release(max_display);
    }
    CHECK_RENDER(render_f32_debug(1.0f), "1.0");
    CHECK_RENDER(render_f32_debug(0.1f), "0.1");
    CHECK_RENDER(render_f32_debug(1e16f), "1e16");
    CHECK_RENDER(render_f32_debug(1e-5f), "1e-5");
    CHECK_RENDER(render_f32_debug(3.4028235e38f), "3.4028235e38");
    CHECK_RENDER(render_f32_debug(-0.0f), "-0.0");

    /* Debug strings are quoted and escaped. */
    {
        LsString* plain = ls_str_cstr("plain");
        LsString* spaced = ls_str_cstr("a\nb\tc\r\"d\\e");
        LsString* control = ls_str_from_parts("x\x01y", 3);
        LsString* utf8 = ls_str_cstr("h\xC3\xA9llo");
        LsString* got;

        got = ls_fmt_debug_string(ls_format_begin(0), plain);
        CHECK_STR(got, "\"plain\"");
        ls_release(got);
        got = ls_fmt_debug_string(ls_format_begin(0), spaced);
        CHECK_STR(got, "\"a\\nb\\tc\\r\\\"d\\\\e\"");
        ls_release(got);
        got = ls_fmt_debug_string(ls_format_begin(0), control);
        CHECK_STR(got, "\"x\\u{1}y\"");
        ls_release(got);
        got = ls_fmt_debug_string(ls_format_begin(0), utf8);
        CHECK_STR(got, "\"h\xC3\xA9llo\"");
        ls_release(got);

        ls_release(plain);
        ls_release(spaced);
        ls_release(control);
        ls_release(utf8);
    }

    /* Formatting a literal via the C-string helper avoids allocation churn. */
    {
        LsString* str = ls_str_cstr("print works");
        ls_println(str);
        ls_release(str);
    }
}

/* ====================================================================== */
/* Format specifiers                                                      */
/* ====================================================================== */

static void check_width(const char* text, int64_t width, int32_t align,
                        uint32_t fill, bool zero_pad, const char* expected,
                        int line) {
    LsString* piece = ls_str_cstr(text);
    LsString* acc = ls_format_begin(8);
    acc = ls_fmt_apply_width(acc, piece, width, align, fill, zero_pad);
    check_str_at(acc, expected, "applied width", line);
    ls_release(acc);
    ls_release(piece);
}

#define CHECK_WIDTH(text, width, align, fill, zero_pad, expected)              \
    check_width((text), (width), (align), (fill), (zero_pad), (expected),      \
                __LINE__)

static void test_format_specs(void) {
    /* "[fill]align" and "[width]" are applied by ls_fmt_apply_width, which
     * counts Unicode scalar values instead of bytes. */
    CHECK_WIDTH("abc", 6, LS_ALIGN_RIGHT, '*', false, "***abc");
    CHECK_WIDTH("abc", 6, LS_ALIGN_LEFT, ' ', false, "abc   ");
    CHECK_WIDTH("abc", 6, LS_ALIGN_CENTER, ' ', false, " abc  ");
    CHECK_WIDTH("中文", 5, LS_ALIGN_RIGHT, '-', false, "---中文");
    CHECK_WIDTH("x", 4, LS_ALIGN_CENTER, 0x4E2DU, false, "中x中中");
    CHECK_WIDTH("x", 4, LS_ALIGN_RIGHT, 0x4E2DU, false, "中中中x");
    /* A piece that is already wide enough is not truncated. */
    CHECK_WIDTH("abc", 2, LS_ALIGN_RIGHT, '*', false, "abc");
    CHECK_WIDTH("abc", 1, LS_ALIGN_RIGHT, '*', false, "abc");
    /* The '0' flag pads with zeros after a sign character. */
    CHECK_WIDTH("7", 4, LS_ALIGN_RIGHT, ' ', true, "0007");
    CHECK_WIDTH("-7", 4, LS_ALIGN_LEFT, ' ', true, "-007");
    CHECK_WIDTH("+5", 5, LS_ALIGN_RIGHT, ' ', true, "+0005");
    CHECK_WIDTH("", 3, LS_ALIGN_CENTER, '.', false, "...");

    /* The '+' sign flag. */
    CHECK_RENDER(ls_fmt_i64_signed(ls_format_begin(0), 42, true), "+42");
    CHECK_RENDER(ls_fmt_i64_signed(ls_format_begin(0), -7, true), "-7");
    CHECK_RENDER(ls_fmt_i64_signed(ls_format_begin(0), 0, true), "+0");
    CHECK_RENDER(ls_fmt_i64_signed(ls_format_begin(0), 42, false), "42");
    CHECK_RENDER(ls_fmt_u64_signed(ls_format_begin(0), 3, false), "3");

    /* "[.precision]" uses fixed notation for floats. */
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), 3.14159, 2, false), "3.14");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), 2.5, 0, false), "2");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), 8.0 / 3.0, 2, false),
                 "2.67");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), 3.0, 1, true), "+3.0");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), -2.0, 3, false), "-2.000");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), 1.0, 2, false), "1.00");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), INFINITY, 2, true),
                 "+inf");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), -INFINITY, 2, false),
                 "-inf");
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), NAN, 2, false), "NaN");
    /* A negative precision means "display formatting". */
    CHECK_RENDER(ls_fmt_f64_prec(ls_format_begin(0), 1.5, -1, false), "1.5");
    CHECK_RENDER(ls_fmt_f32_prec(ls_format_begin(0), 0.1f, 3, false), "0.100");

    /* "[.precision]" truncates strings on a UTF-8 boundary. */
    {
        LsString* ascii = ls_str_cstr("abcdef");
        LsString* utf8 = ls_str_cstr("你好世界");
        LsString* empty = ls_str_cstr("");

        CHECK_RENDER(ls_fmt_str_prec(ls_format_begin(0), ascii, 3), "abc");
        CHECK_RENDER(ls_fmt_str_prec(ls_format_begin(0), ascii, 0), "");
        CHECK_RENDER(ls_fmt_str_prec(ls_format_begin(0), ascii, 10), "abcdef");
        CHECK_RENDER(ls_fmt_str_prec(ls_format_begin(0), ascii, -1), "abcdef");
        CHECK_RENDER(ls_fmt_str_prec(ls_format_begin(0), utf8, 2), "你好");
        CHECK_RENDER(ls_fmt_str_prec(ls_format_begin(0), utf8, 10), "你好世界");
        CHECK_RENDER(ls_fmt_str_prec(ls_format_begin(0), empty, 4), "");

        ls_release(ascii);
        ls_release(utf8);
        ls_release(empty);
    }
}

/* ====================================================================== */
/* Frozen shared instantiations                                           */
/* ====================================================================== */

static void test_option_result(void) {
    LsOpt_String none = ls_opt_string_none();
    LsOpt_String some = ls_opt_string_some(ls_str_cstr("value"));
    LsRes_String_String ok = ls_res_string_ok(ls_str_cstr("ok-value"));
    LsRes_String_String err = ls_res_string_err(ls_str_cstr("err-value"));

    CHECK(none.tag == 0);
    CHECK(none.u.some == NULL);
    CHECK(some.tag == 1);
    CHECK_STR(some.u.some, "value");
    CHECK(ok.tag == 0);
    CHECK_STR(ok.u.ok, "ok-value");
    CHECK(err.tag == 1);
    CHECK_STR(err.u.err, "err-value");

    ls_release(some.u.some);
    ls_release(ok.u.ok);
    ls_release(err.u.err);
}

/* ====================================================================== */
/* Checked arithmetic and math                                            */
/* ====================================================================== */

static bool nearly(double a, double b, double tolerance) {
    return fabs(a - b) <= tolerance;
}

static void test_checked_math(void) {
    CHECK(ls_add_i8_checked(100, 27, TEST_FILE, 1, 1) == 127);
    CHECK(ls_add_i8_checked(-100, 27, TEST_FILE, 1, 1) == -73);
    CHECK(ls_sub_i16_checked(100, 27, TEST_FILE, 1, 1) == 73);
    CHECK(ls_mul_i32_checked(-6, 7, TEST_FILE, 1, 1) == -42);
    CHECK(ls_div_i64_checked(-7, 2, TEST_FILE, 1, 1) == -3);
    CHECK(ls_rem_i64_checked(-7, 2, TEST_FILE, 1, 1) == -1);
    CHECK(ls_shl_i32_checked(1, 31, TEST_FILE, 1, 1) == INT32_MIN);
    CHECK(ls_shr_i32_checked(-8, 1, TEST_FILE, 1, 1) == -4);
    CHECK(ls_neg_i8_checked(-127, TEST_FILE, 1, 1) == 127);

    CHECK(ls_add_u8_checked(200, 55, TEST_FILE, 1, 1) == 255);
    CHECK(ls_sub_u16_checked(100, 27, TEST_FILE, 1, 1) == 73);
    CHECK(ls_mul_u32_checked(60000, 60000, TEST_FILE, 1, 1) == 3600000000u);
    CHECK(ls_div_u64_checked(9, 2, TEST_FILE, 1, 1) == 4);
    CHECK(ls_rem_u64_checked(9, 2, TEST_FILE, 1, 1) == 1);
    CHECK(ls_shl_u64_checked(1, 63, TEST_FILE, 1, 1) ==
          ((uint64_t)1 << 63));
    CHECK(ls_shr_u64_checked(UINT64_MAX, 63, TEST_FILE, 1, 1) == 1);

    CHECK(ls_abs_i32(-5) == 5);
    CHECK(ls_abs_i64(INT64_MIN) == INT64_MIN); /* wrapping_abs */
    CHECK(ls_abs_i8_checked(-5, TEST_FILE, 1, 1) == 5);
    CHECK(ls_abs_i64_checked(-5, TEST_FILE, 1, 1) == 5);
    CHECK(ls_min_i32(-3, 4) == -3);
    CHECK(ls_max_i32(-3, 4) == 4);
    CHECK(ls_min_u64(3, 4) == 3);
    CHECK(ls_max_u64(3, 4) == 4);
    CHECK(ls_abs_u32(42) == 42);

    CHECK(nearly(ls_sqrt_f64(4.0), 2.0, 1e-12));
    CHECK(nearly(ls_sqrt_f32(9.0f), 3.0f, 1e-6));
    CHECK(nearly(ls_pow_f64(2.0, 10.0), 1024.0, 1e-9));
    CHECK(nearly(ls_pow_f32(2.0f, 10.0f), 1024.0f, 1e-3));
    CHECK(ls_floor_f64(-1.5) == -2.0);
    CHECK(ls_ceil_f64(-1.5) == -1.0);
    CHECK(ls_round_f64(2.5) == 3.0);
    CHECK(ls_round_f64(-2.5) == -3.0);
    CHECK(ls_floor_f32(2.9f) == 2.0f);
    CHECK(ls_ceil_f32(2.1f) == 3.0f);
    CHECK(ls_round_f32(2.5f) == 3.0f);
    CHECK(nearly(ls_sin_f64(0.0), 0.0, 1e-12));
    CHECK(nearly(ls_cos_f64(0.0), 1.0, 1e-12));
    CHECK(nearly(ls_tan_f64(0.0), 0.0, 1e-12));
    CHECK(nearly(ls_ln_f64(1.0), 0.0, 1e-12));
    CHECK(nearly(ls_log10_f64(1000.0), 3.0, 1e-12));
    CHECK(nearly(ls_exp_f64(0.0), 1.0, 1e-12));
    CHECK(ls_abs_f64(-2.5) == 2.5);
    CHECK(ls_abs_f32(-2.5f) == 2.5f);
    CHECK(ls_min_f64(1.0, 2.0) == 1.0);
    CHECK(ls_max_f64(1.0, 2.0) == 2.0);
    CHECK(ls_min_f32(1.0f, 2.0f) == 1.0f);
    CHECK(ls_max_f32(1.0f, 2.0f) == 2.0f);
}

/* ====================================================================== */
/* File IO (only in --io mode)                                            */
/* ====================================================================== */

/* Error paths of the file helpers. The path does not have to be writable. */
static void test_io_errors(const char* path) {
    if (path == NULL) {
        return;
    }
    {
        char missing[1024];
        LsString* missing_path;
        LsRes_String_String result;
        (void)snprintf(missing, sizeof missing, "%s.missing", path);
        missing_path = ls_str_cstr(missing);
        result = ls_read_file(missing_path);
        CHECK(result.tag == 1);
        if (result.tag == 1) {
            CHECK(ls_str_len(result.u.err) > 0);
            CHECK(contains_cstr(result.u.err, "os error"));
            ls_release(result.u.err);
        }
        ls_release(missing_path);
    }
    {
        char bad[1024];
        LsString* bad_path;
        LsString* contents = ls_str_cstr("x");
        LsRes_String_String result;
        (void)snprintf(bad, sizeof bad, "%s.dir/denied.txt", path);
        bad_path = ls_str_cstr(bad);
        result = ls_write_file(bad_path, contents);
        CHECK(result.tag == 1);
        if (result.tag == 1) {
            CHECK(ls_str_len(result.u.err) > 0);
            CHECK(contains_cstr(result.u.err, "os error"));
            ls_release(result.u.err);
        }
        ls_release(bad_path);
        ls_release(contents);
    }
}

static void test_io(const char* path) {
    if (path == NULL) {
        return;
    }
    {
        const char raw[] = {'h', 'i', '\0', '!', '\n'};
        LsString* file_path = ls_str_cstr(path);
        LsString* contents = ls_str_from_parts(raw, 5);
        LsRes_String_String written = ls_write_file(file_path, contents);
        LsRes_String_String read_back;

        CHECK(written.tag == 0);
        if (written.tag == 0) {
            ls_release(written.u.ok);
        }
        read_back = ls_read_file(file_path);
        CHECK(read_back.tag == 0);
        if (read_back.tag == 0) {
            CHECK(ls_str_len(read_back.u.ok) == 5);
            CHECK(memcmp(ls_str_data(read_back.u.ok), raw, 5) == 0);
            ls_release(read_back.u.ok);
        }
        ls_release(contents);
        ls_release(file_path);
    }
    test_io_errors(path);
}

/* ====================================================================== */
/* Hash maps                                                              */
/* ====================================================================== */

static int g_map_key_copies = 0;
static int g_map_key_drops = 0;
static int g_map_value_copies = 0;
static int g_map_value_drops = 0;

/* Scalar key/value callbacks. The runtime never calls copy_key(); the counter
 * exists so the tests can prove it. */
static void map_copy_key_i64(void* out, const void* in) {
    memcpy(out, in, sizeof(int64_t));
    g_map_key_copies += 1;
}

static void map_drop_key_i64(void* key_bytes) {
    (void)key_bytes;
    g_map_key_drops += 1;
}

static void map_copy_value_i64(void* out, const void* in) {
    memcpy(out, in, sizeof(int64_t));
    g_map_value_copies += 1;
}

static void map_drop_value_i64(void* value_bytes) {
    (void)value_bytes;
    g_map_value_drops += 1;
}

static void map_copy_key_u64(void* out, const void* in) {
    memcpy(out, in, sizeof(uint64_t));
    g_map_key_copies += 1;
}

static void map_drop_key_u64(void* key_bytes) {
    (void)key_bytes;
    g_map_key_drops += 1;
}

static void map_copy_key_bool(void* out, const void* in) {
    memcpy(out, in, sizeof(bool));
    g_map_key_copies += 1;
}

static void map_drop_key_bool(void* key_bytes) {
    (void)key_bytes;
    g_map_key_drops += 1;
}

static void map_copy_key_string(void* out, const void* in) {
    LsString* handle;
    memcpy(&handle, in, sizeof handle);
    handle = (LsString*)ls_retain(handle);
    memcpy(out, &handle, sizeof handle);
    g_map_key_copies += 1;
}

static void map_drop_key_string(void* key_bytes) {
    LsString* handle;
    memcpy(&handle, key_bytes, sizeof handle);
    if (handle != NULL) {
        ls_release(handle);
    }
    g_map_key_drops += 1;
}

static void map_copy_value_string(void* out, const void* in) {
    LsString* handle;
    memcpy(&handle, in, sizeof handle);
    handle = (LsString*)ls_retain(handle);
    memcpy(out, &handle, sizeof handle);
    g_map_value_copies += 1;
}

static void map_drop_value_string(void* value_bytes) {
    LsString* handle;
    memcpy(&handle, value_bytes, sizeof handle);
    if (handle != NULL) {
        ls_release(handle);
    }
    g_map_value_drops += 1;
}

static void map_init_i64_i64(LsMapTable* table) {
    ls_map_init(table, sizeof(int64_t), sizeof(int64_t), ls_hash_i64, ls_eq_i64,
                map_copy_key_i64, map_drop_key_i64, map_copy_value_i64,
                map_drop_value_i64);
}

static void map_init_i64_string(LsMapTable* table) {
    ls_map_init(table, sizeof(int64_t), sizeof(LsString*), ls_hash_i64,
                ls_eq_i64, map_copy_key_i64, map_drop_key_i64,
                map_copy_value_string, map_drop_value_string);
}

static void map_init_string_i64(LsMapTable* table) {
    ls_map_init(table, sizeof(LsString*), sizeof(int64_t), ls_hash_string_slot,
                ls_eq_string_slot, map_copy_key_string, map_drop_key_string,
                map_copy_value_i64, map_drop_value_i64);
}

static void map_init_u64_i64(LsMapTable* table) {
    ls_map_init(table, sizeof(uint64_t), sizeof(int64_t), ls_hash_u64, ls_eq_u64,
                map_copy_key_u64, map_drop_key_u64, map_copy_value_i64,
                map_drop_value_i64);
}

static void map_init_bool_i64(LsMapTable* table) {
    ls_map_init(table, sizeof(bool), sizeof(int64_t), ls_hash_bool, ls_eq_bool,
                map_copy_key_bool, map_drop_key_bool, map_copy_value_i64,
                map_drop_value_i64);
}

/* Control-byte values and constants of the Swiss-table layout (see
 * lsaca_rt_map.c and the LsMapTable documentation in lsaca_rt.h). The table
 * struct is public, so the adversarial tests below forge control bytes with
 * these values. */
#define MAP_CTRL_EMPTY_BYTE ((unsigned char)0x80)
#define MAP_CTRL_DELETED_BYTE ((unsigned char)0xFE)
#define MAP_CTRL_H2_LIMIT ((unsigned char)0x7F)
#define MAP_CTRL_GROUP_WIDTH ((size_t)16)
#define MAP_CTRL_LOAD_DENOMINATOR ((size_t)8)

/* Frees the three slot arrays; the table must be empty (cleared) first. */
static void map_free_arrays(LsMapTable* table) {
    free(table->keys);
    free(table->values);
    free(table->ctrl);
    table->keys = NULL;
    table->values = NULL;
    table->ctrl = NULL;
    table->cap = 0;
    table->len = 0;
    table->growth_left = 0;
}

/* Checks the Swiss-table invariants the runtime documents for LsMapTable:
 * capacity is a power of two >= 16 (or 0 before the first insert), the three
 * arrays exist together, the load factor stays at most 7/8, every control byte
 * is EMPTY (0x80), DELETED (0xFE) or an occupied H2 (0x00..0x7F), the occupied
 * byte count equals len, the EMPTY count equals cap/8 + growth_left, and the
 * 16-byte mirror after the table repeats the first control group. */
static bool map_capacity_ok(const LsMapTable* table) {
    size_t cap = table->cap;
    size_t occupied = 0;
    size_t empty = 0;
    size_t index;

    if (cap == 0) {
        return table->len == 0 && table->keys == NULL &&
               table->values == NULL && table->ctrl == NULL;
    }
    if ((cap & (cap - 1)) != 0 || cap < MAP_CTRL_GROUP_WIDTH) {
        return false;
    }
    if (table->keys == NULL || table->values == NULL ||
        table->ctrl == NULL) {
        return false;
    }
    if (table->len * 8 > cap * 7) {
        return false;
    }
    if (table->growth_left > cap - cap / MAP_CTRL_LOAD_DENOMINATOR) {
        return false;
    }
    for (index = 0; index < cap; index += 1) {
        unsigned char byte = table->ctrl[index];
        if (byte == MAP_CTRL_EMPTY_BYTE) {
            empty += 1;
        } else if (byte == MAP_CTRL_DELETED_BYTE) {
            /* Tombstones are not occupied entries. */
        } else if (byte <= MAP_CTRL_H2_LIMIT) {
            occupied += 1;
        } else {
            return false;
        }
    }
    if (occupied != table->len) {
        return false;
    }
    if (empty != cap / MAP_CTRL_LOAD_DENOMINATOR + table->growth_left) {
        return false;
    }
    return memcmp(table->ctrl + cap, table->ctrl, MAP_CTRL_GROUP_WIDTH) == 0;
}

static bool map_sequences_equal(const LsMapTable* a, const LsMapTable* b) {
    LsMapIter ia;
    LsMapIter ib;
    ls_map_iter_init(&ia, a);
    ls_map_iter_init(&ib, b);
    for (;;) {
        const void* key_a = NULL;
        const void* value_a = NULL;
        const void* key_b = NULL;
        const void* value_b = NULL;
        bool more_a = ls_map_iter_next(&ia, &key_a, &value_a);
        bool more_b = ls_map_iter_next(&ib, &key_b, &value_b);
        if (more_a != more_b) {
            return false;
        }
        if (!more_a) {
            return true;
        }
        if (memcmp(key_a, key_b, sizeof(int64_t)) != 0) {
            return false;
        }
        if (memcmp(value_a, value_b, sizeof(int64_t)) != 0) {
            return false;
        }
    }
}

static void test_map_hash_primitives(void) {
    int64_t zero = 0;
    int64_t one = 1;
    int64_t forty_two = 42;
    int64_t same = 42;
    int64_t different = 43;
    int64_t minus_one = -1;
    uint64_t uzero = 0;
    uint64_t uone = 1;
    uint64_t usame = 1;
    uint64_t udifferent = 2;
    bool bfalse = false;
    bool btrue = true;
    LsString* null_handle = NULL;
    LsString* first = ls_str_cstr("abc");
    LsString* second = ls_str_cstr("abc");
    LsString* third = ls_str_cstr("hello world");

    CHECK(first != NULL && second != NULL && third != NULL);
    /* wyhash-style 64-bit hash with fixed secrets: frozen constants (see
     * lsaca_rt_map.c). Scalar helpers hash the little-endian byte pattern and
     * agree bit for bit with ls_hash_bytes over the same bytes, so a zero
     * scalar still hashes non-zero. */
    CHECK(ls_hash_bytes("", 0) == 0x09EC1EB68F0BB84AULL);
    CHECK(ls_hash_bytes(NULL, 0) == 0x09EC1EB68F0BB84AULL);
    CHECK(ls_hash_bytes("abc", 3) == 0xC290F8A784C43C92ULL);
    CHECK(ls_hash_bytes("hello world", 11) == 0x279192601F8FDC93ULL);
    CHECK(ls_hash_bytes("abc", 3) != ls_hash_bytes("abd", 3));
    CHECK(ls_hash_i64(&zero) == 0xE57D83D407CABA2FULL);
    CHECK(ls_hash_i64(&one) == 0x2E07CAD7E5281828ULL);
    CHECK(ls_hash_i64(&forty_two) == 0xAC8C9D001F291713ULL);
    CHECK(ls_hash_i64(&minus_one) == 0xBB0FCCAA641DF4B6ULL);
    CHECK(ls_hash_i64(&forty_two) == ls_hash_i64(&same));
    CHECK(ls_eq_i64(&forty_two, &same));
    CHECK(!ls_eq_i64(&forty_two, &different));
    CHECK(ls_hash_u64(&uzero) == 0xE57D83D407CABA2FULL);
    CHECK(ls_hash_u64(&uone) == 0x2E07CAD7E5281828ULL);
    CHECK(ls_eq_u64(&uone, &usame));
    CHECK(!ls_eq_u64(&uone, &udifferent));
    CHECK(ls_hash_bool(&bfalse) == 0xF41EDC109ABBA57FULL);
    CHECK(ls_hash_bool(&btrue) == 0x05BD9307C048752EULL);
    CHECK(ls_eq_bool(&btrue, &btrue));
    CHECK(ls_eq_bool(&bfalse, &bfalse));
    CHECK(!ls_eq_bool(&btrue, &bfalse));
    CHECK(ls_hash_string_slot(&null_handle) == 0x09EC1EB68F0BB84AULL);
    CHECK(ls_eq_string_slot(&null_handle, &null_handle));
    CHECK(ls_hash_string_slot(&first) == 0xC290F8A784C43C92ULL);
    CHECK(ls_hash_string_slot(&first) == ls_hash_string_slot(&second));
    CHECK(ls_eq_string_slot(&first, &second));
    CHECK(!ls_eq_string_slot(&first, &third));
    CHECK(!ls_eq_string_slot(&first, &null_handle));
    CHECK(!ls_eq_string_slot(&null_handle, &first));
    CHECK(ls_hash_string_slot(&third) == 0x279192601F8FDC93ULL);
    ls_release(first);
    ls_release(second);
    ls_release(third);
}

static void test_map_int_basics(void) {
    LsMapTable table;
    void* slot = NULL;
    int64_t key;
    int64_t value;
    int64_t out_old = 0;
    size_t i;

    map_init_i64_i64(&table);
    CHECK(table.cap == 0);
    CHECK(table.len == 0);
    CHECK(table.key_size == sizeof(int64_t));
    CHECK(table.value_size == sizeof(int64_t));
    CHECK(table.keys == NULL && table.values == NULL && table.ctrl == NULL);
    CHECK(map_capacity_ok(&table));
    CHECK(ls_map_len(&table) == 0);
    CHECK(ls_map_is_empty(&table));

    key = 0;
    CHECK(!ls_map_get_ptr(&table, &key, &slot));
    CHECK(slot == NULL);

    key = 7;
    value = 70;
    CHECK(!ls_map_insert(&table, &key, &value, &out_old));
    CHECK(table.len == 1);
    CHECK(table.cap == 16);
    CHECK(map_capacity_ok(&table));
    CHECK(!ls_map_is_empty(&table));
    CHECK(ls_map_get_ptr(&table, &key, &slot));
    CHECK(slot != NULL && *(const int64_t*)slot == 70);

    value = 71;
    CHECK(ls_map_insert(&table, &key, &value, &out_old));
    CHECK(out_old == 70);
    CHECK(ls_map_get_ptr(&table, &key, &slot));
    CHECK(*(const int64_t*)slot == 71);
    CHECK(table.len == 1);

    value = 72;
    /* out_old_value is NULL, so no old value is returned and the result is
     * false even though the key exists. */
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    CHECK(ls_map_get_ptr(&table, &key, &slot));
    CHECK(*(const int64_t*)slot == 72);

    {
        int64_t missing = 8;
        CHECK(!ls_map_get_ptr(&table, &missing, &slot));
    }

    out_old = -1;
    CHECK(ls_map_remove(&table, &key, &out_old));
    CHECK(out_old == 72);
    CHECK(table.len == 0);
    CHECK(ls_map_is_empty(&table));
    CHECK(!ls_map_get_ptr(&table, &key, &slot));
    CHECK(!ls_map_remove(&table, &key, &out_old));

    key = 9;
    value = 90;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    CHECK(ls_map_remove(&table, &key, NULL));
    CHECK(table.len == 0);

    for (i = 0; i < 10; i += 1) {
        key = (int64_t)(i + 1);
        value = key * 10;
        CHECK(!ls_map_insert(&table, &key, &value, NULL));
    }
    CHECK(table.len == 10);
    /* A fresh table starts at capacity 16; the 7/8 load factor holds 14
     * entries, so ten entries fit without a grow. */
    CHECK(table.cap == 16);
    CHECK(map_capacity_ok(&table));
    for (i = 0; i < 10; i += 1) {
        key = (int64_t)(i + 1);
        CHECK(ls_map_get_ptr(&table, &key, &slot));
        CHECK(*(const int64_t*)slot == key * 10);
    }

    ls_map_clear(&table);
    CHECK(table.len == 0);
    CHECK(ls_map_is_empty(&table));
    CHECK(table.cap == 16);
    CHECK(map_capacity_ok(&table));
    for (i = 0; i < 10; i += 1) {
        key = (int64_t)(i + 1);
        CHECK(!ls_map_get_ptr(&table, &key, &slot));
    }

    key = 3;
    value = 33;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    CHECK(ls_map_get_ptr(&table, &key, &slot));
    CHECK(*(const int64_t*)slot == 33);
    ls_map_clear(&table);
    map_free_arrays(&table);
    CHECK(map_capacity_ok(&table));
}

static void test_map_iteration_order(void) {
    /* Frozen iteration order for i64 keys 1..5 in a capacity-16 table: the
     * table has a single 16-slot group, so entries come back in insertion
     * order. */
    static const int64_t expected[5] = {1, 2, 3, 4, 5};
    LsMapTable table;
    LsMapIter iter;
    int64_t keys[5];
    size_t i;
    size_t seen = 0;

    map_init_i64_i64(&table);
    for (i = 0; i < 5; i += 1) {
        int64_t value;
        keys[i] = (int64_t)(i + 1);
        value = keys[i] * 100;
        CHECK(!ls_map_insert(&table, &keys[i], &value, NULL));
    }
    CHECK(table.cap == 16);
    ls_map_iter_init(&iter, &table);
    for (;;) {
        const void* key_ptr = NULL;
        const void* value_ptr = NULL;
        if (!ls_map_iter_next(&iter, &key_ptr, &value_ptr)) {
            break;
        }
        CHECK(seen < 5);
        if (seen < 5) {
            CHECK(*(const int64_t*)key_ptr == expected[seen]);
            CHECK(*(const int64_t*)value_ptr == expected[seen] * 100);
        }
        seen += 1;
    }
    CHECK(seen == 5);

    /* NULL output pointers are allowed and still advance the iterator. */
    ls_map_iter_init(&iter, &table);
    seen = 0;
    while (ls_map_iter_next(&iter, NULL, NULL)) {
        seen += 1;
    }
    CHECK(seen == 5);
    map_free_arrays(&table);
}

static void test_map_growth_iteration(void) {
    LsMapTable table;
    LsMapTable twin;
    LsMapIter iter;
    size_t i;
    size_t count;
    size_t removed = 0;
    int64_t sum;

    map_init_i64_i64(&table);
    for (i = 0; i < 1000; i += 1) {
        int64_t key = (int64_t)i;
        int64_t value = key * 3;
        CHECK(!ls_map_insert(&table, &key, &value, NULL));
    }
    CHECK(table.len == 1000);
    CHECK(table.cap == 2048);
    CHECK(map_capacity_ok(&table));
    sum = 0;
    for (i = 0; i < 1000; i += 1) {
        int64_t key = (int64_t)i;
        void* slot = NULL;
        CHECK(ls_map_get_ptr(&table, &key, &slot));
        if (slot != NULL) {
            sum += *(const int64_t*)slot;
        }
    }
    CHECK(sum == 1498500);
    count = 0;
    sum = 0;
    ls_map_iter_init(&iter, &table);
    for (;;) {
        const void* key_ptr = NULL;
        const void* value_ptr = NULL;
        if (!ls_map_iter_next(&iter, &key_ptr, &value_ptr)) {
            break;
        }
        count += 1;
        sum += *(const int64_t*)key_ptr;
        CHECK(*(const int64_t*)value_ptr == *(const int64_t*)key_ptr * 3);
    }
    CHECK(count == 1000);
    CHECK(sum == 499500);

    /* A second table built by the same operations iterates identically. */
    map_init_i64_i64(&twin);
    for (i = 0; i < 1000; i += 1) {
        int64_t key = (int64_t)i;
        int64_t value = key * 3;
        CHECK(!ls_map_insert(&twin, &key, &value, NULL));
    }
    CHECK(map_sequences_equal(&table, &twin));

    /* Remove every third key; the resulting tombstones must not break probing. */
    for (i = 0; i < 1000; i += 3) {
        int64_t key = (int64_t)i;
        int64_t out_value = -1;
        CHECK(ls_map_remove(&table, &key, &out_value));
        CHECK(out_value == key * 3);
        removed += 1;
    }
    CHECK(removed == 334);
    CHECK(table.len == 666);
    CHECK(map_capacity_ok(&table));
    for (i = 0; i < 1000; i += 1) {
        int64_t key = (int64_t)i;
        void* slot = NULL;
        bool present = ls_map_get_ptr(&table, &key, &slot);
        CHECK(present == ((i % 3) != 0));
        if (present) {
            CHECK(*(const int64_t*)slot == key * 3);
        }
    }
    count = 0;
    sum = 0;
    ls_map_iter_init(&iter, &table);
    for (;;) {
        const void* key_ptr = NULL;
        if (!ls_map_iter_next(&iter, &key_ptr, NULL)) {
            break;
        }
        count += 1;
        sum += *(const int64_t*)key_ptr;
    }
    CHECK(count == 666);
    CHECK(sum == 332667);

    /* Inserting past the load factor grows the table again; tombstones are
     * rehashed away and the old entries survive. */
    for (i = 1000; i < 2000; i += 1) {
        int64_t key = (int64_t)i;
        int64_t value = key * 2;
        CHECK(!ls_map_insert(&table, &key, &value, NULL));
    }
    CHECK(table.len == 1666);
    CHECK(map_capacity_ok(&table));
    for (i = 1000; i < 2000; i += 1) {
        int64_t key = (int64_t)i;
        void* slot = NULL;
        CHECK(ls_map_get_ptr(&table, &key, &slot));
        if (slot != NULL) {
            CHECK(*(const int64_t*)slot == key * 2);
        }
    }
    sum = 0;
    ls_map_iter_init(&iter, &table);
    for (;;) {
        const void* key_ptr = NULL;
        const void* value_ptr = NULL;
        if (!ls_map_iter_next(&iter, &key_ptr, &value_ptr)) {
            break;
        }
        (void)value_ptr;
        sum += *(const int64_t*)key_ptr;
    }
    CHECK(sum == 1832167);
    map_free_arrays(&table);
    map_free_arrays(&twin);
}

static void test_map_key_types(void) {
    LsMapTable u64_map;
    LsMapTable bool_map;
    uint64_t ukeys[4] = {0, 1, 2, UINT64_MAX};
    bool bkeys[2] = {false, true};
    size_t i;

    map_init_u64_i64(&u64_map);
    for (i = 0; i < 4; i += 1) {
        int64_t value = (int64_t)(i * 11 + 1);
        CHECK(!ls_map_insert(&u64_map, &ukeys[i], &value, NULL));
    }
    CHECK(u64_map.len == 4);
    CHECK(map_capacity_ok(&u64_map));
    for (i = 0; i < 4; i += 1) {
        void* slot = NULL;
        CHECK(ls_map_get_ptr(&u64_map, &ukeys[i], &slot));
        if (slot != NULL) {
            CHECK(*(const int64_t*)slot == (int64_t)(i * 11 + 1));
        }
    }
    {
        uint64_t missing = 3;
        void* slot = NULL;
        CHECK(!ls_map_get_ptr(&u64_map, &missing, &slot));
        CHECK(!ls_map_remove(&u64_map, &missing, NULL));
    }
    {
        int64_t removed = 0;
        CHECK(ls_map_remove(&u64_map, &ukeys[3], &removed));
        CHECK(removed == 34);
    }
    CHECK(u64_map.len == 3);
    ls_map_clear(&u64_map);
    map_free_arrays(&u64_map);

    map_init_bool_i64(&bool_map);
    {
        int64_t value_false = 100;
        int64_t value_true = 200;
        CHECK(!ls_map_insert(&bool_map, &bkeys[0], &value_false, NULL));
        CHECK(!ls_map_insert(&bool_map, &bkeys[1], &value_true, NULL));
    }
    CHECK(bool_map.len == 2);
    CHECK(map_capacity_ok(&bool_map));
    {
        void* slot = NULL;
        CHECK(ls_map_get_ptr(&bool_map, &bkeys[0], &slot));
        CHECK(slot != NULL && *(const int64_t*)slot == 100);
        slot = NULL;
        CHECK(ls_map_get_ptr(&bool_map, &bkeys[1], &slot));
        CHECK(slot != NULL && *(const int64_t*)slot == 200);
    }
    {
        int64_t out_value = 0;
        CHECK(ls_map_remove(&bool_map, &bkeys[1], &out_value));
        CHECK(out_value == 200);
        CHECK(!ls_map_remove(&bool_map, &bkeys[1], &out_value));
    }
    CHECK(bool_map.len == 1);
    ls_map_clear(&bool_map);
    map_free_arrays(&bool_map);
}

static void test_map_string_keys(void) {
    LsMapTable table;
    LsString* stored;
    LsString* probe;
    LsString* keep;
    int64_t value;
    int64_t out_value;
    void* slot = NULL;

    g_map_key_copies = 0;
    g_map_key_drops = 0;
    g_map_value_copies = 0;
    g_map_value_drops = 0;

    map_init_string_i64(&table);
    stored = ls_str_cstr("alpha");
    CHECK(stored != NULL);
    CHECK(ls_refcount(stored) == 1);
    value = 10;
    /* Adoption: the map takes over the caller's reference, no callback runs. */
    CHECK(!ls_map_insert(&table, &stored, &value, NULL));
    CHECK(table.len == 1);
    CHECK(ls_refcount(stored) == 1);
    CHECK(g_map_key_copies == 0);
    CHECK(g_map_key_drops == 0);
    CHECK(g_map_value_copies == 0);
    CHECK(g_map_value_drops == 0);

    probe = ls_str_cstr("alpha");
    CHECK(probe != NULL && probe != stored);
    CHECK(ls_map_get_ptr(&table, &probe, &slot));
    CHECK(slot != NULL && *(const int64_t*)slot == 10);
    keep = (LsString*)ls_retain(probe);
    CHECK(keep == probe);
    CHECK(ls_refcount(probe) == 2);
    /* Overwriting releases the caller's duplicate key through drop_key(). */
    value = 20;
    CHECK(!ls_map_insert(&table, &probe, &value, NULL));
    CHECK(ls_refcount(probe) == 1);
    CHECK(g_map_key_drops == 1);
    CHECK(g_map_value_drops == 1);
    ls_release(keep);

    /* Overwriting with an old-value output: copy_value retains, drop_value
     * releases the stored value. */
    probe = ls_str_cstr("alpha");
    CHECK(probe != NULL);
    keep = (LsString*)ls_retain(probe);
    CHECK(keep == probe);
    CHECK(ls_refcount(probe) == 2);
    value = 30;
    out_value = 0;
    CHECK(ls_map_insert(&table, &probe, &value, &out_value));
    CHECK(out_value == 20);
    CHECK(ls_refcount(probe) == 1);
    CHECK(g_map_value_copies == 1);
    CHECK(g_map_value_drops == 2);
    CHECK(g_map_key_drops == 2);
    ls_release(keep);

    /* A retained observer keeps a stored key observable across removal. */
    {
        LsString* observer = ls_str_cstr("gamma");
        LsString* observer_probe;
        int64_t gamma_value = 7;
        CHECK(observer != NULL);
        CHECK(!ls_map_insert(&table, &observer, &gamma_value, NULL));
        CHECK(ls_refcount(observer) == 1);
        CHECK(ls_retain(observer) == observer);
        CHECK(ls_refcount(observer) == 2);
        observer_probe = ls_str_cstr("gamma");
        CHECK(observer_probe != NULL);
        CHECK(ls_map_get_ptr(&table, &observer_probe, &slot));
        CHECK(slot != NULL && *(const int64_t*)slot == 7);
        ls_release(observer_probe);
        CHECK(ls_map_remove(&table, &observer, &out_value));
        CHECK(out_value == 7);
        CHECK(ls_refcount(observer) == 1);
        ls_release(observer);
    }

    /* clear() releases every stored key and value, and the table stays usable. */
    {
        LsString* beta = ls_str_cstr("beta");
        LsString* delta = ls_str_cstr("delta");
        int64_t beta_value = 2;
        int64_t delta_value = 4;
        int key_drops_before = g_map_key_drops;
        int value_drops_before = g_map_value_drops;
        CHECK(beta != NULL && delta != NULL);
        CHECK(!ls_map_insert(&table, &beta, &beta_value, NULL));
        CHECK(!ls_map_insert(&table, &delta, &delta_value, NULL));
        CHECK(table.len == 3);
        ls_map_clear(&table);
        CHECK(table.len == 0);
        CHECK(ls_map_is_empty(&table));
        CHECK(g_map_key_drops == key_drops_before + 3);
        CHECK(g_map_value_drops == value_drops_before + 3);
        {
            LsString* again = ls_str_cstr("alpha");
            int64_t again_value = 1;
            CHECK(again != NULL);
            CHECK(!ls_map_insert(&table, &again, &again_value, NULL));
            CHECK(table.len == 1);
            ls_map_clear(&table);
        }
    }
    CHECK(g_map_key_copies == 0);
    map_free_arrays(&table);
}

static void test_map_string_values(void) {
    LsMapTable table;
    LsString* first;
    LsString* second;
    LsString* third;
    LsString* out_old = NULL;
    LsString* removed = NULL;
    int64_t key = 1;
    void* slot = NULL;
    int value_copies_before;
    int value_drops_before;
    int key_drops_before;

    g_map_key_copies = 0;
    g_map_key_drops = 0;
    g_map_value_copies = 0;
    g_map_value_drops = 0;

    map_init_i64_string(&table);
    first = ls_str_cstr("first");
    second = ls_str_cstr("second");
    CHECK(first != NULL && second != NULL);
    CHECK(!ls_map_insert(&table, &key, &first, &out_old));
    CHECK(out_old == NULL);
    CHECK(ls_refcount(first) == 1);
    CHECK(ls_map_get_ptr(&table, &key, &slot));
    CHECK(slot != NULL && *(LsString**)slot == first);
    CHECK(g_map_value_copies == 0);
    CHECK(g_map_value_drops == 0);

    /* Overwrite: copy_value retains the old value into out_old, drop_value
     * releases the map's old reference. */
    value_copies_before = g_map_value_copies;
    value_drops_before = g_map_value_drops;
    CHECK(ls_map_insert(&table, &key, &second, &out_old));
    CHECK(out_old == first);
    CHECK(ls_refcount(out_old) == 1);
    CHECK(ls_refcount(second) == 1);
    ls_release(out_old);
    out_old = NULL;
    CHECK(g_map_value_copies == value_copies_before + 1);
    CHECK(g_map_value_drops == value_drops_before + 1);

    /* Overwrite without an old value: only drop_value runs. */
    third = ls_str_cstr("third");
    CHECK(third != NULL);
    value_copies_before = g_map_value_copies;
    value_drops_before = g_map_value_drops;
    CHECK(!ls_map_insert(&table, &key, &third, NULL));
    CHECK(ls_refcount(third) == 1);
    CHECK(g_map_value_copies == value_copies_before);
    CHECK(g_map_value_drops == value_drops_before + 1);

    /* remove moves the value out. */
    key_drops_before = g_map_key_drops;
    value_drops_before = g_map_value_drops;
    CHECK(ls_map_remove(&table, &key, &removed));
    CHECK(removed == third);
    CHECK(ls_refcount(removed) == 1);
    CHECK(g_map_key_drops == key_drops_before + 1);
    CHECK(g_map_value_drops == value_drops_before);
    ls_release(removed);
    CHECK(ls_map_is_empty(&table));

    /* remove without an output releases the value. */
    {
        LsString* fourth = ls_str_cstr("fourth");
        int64_t other_key = 2;
        CHECK(fourth != NULL);
        CHECK(!ls_map_insert(&table, &other_key, &fourth, NULL));
        value_drops_before = g_map_value_drops;
        CHECK(ls_map_remove(&table, &other_key, NULL));
        CHECK(g_map_value_drops == value_drops_before + 1);
    }

    /* clear drops stored values and the table stays usable. */
    {
        LsString* fifth = ls_str_cstr("fifth");
        int64_t third_key = 3;
        CHECK(fifth != NULL);
        CHECK(!ls_map_insert(&table, &third_key, &fifth, NULL));
        value_drops_before = g_map_value_drops;
        ls_map_clear(&table);
        CHECK(g_map_value_drops == value_drops_before + 1);
    }
    CHECK(g_map_key_copies == 0);
    map_free_arrays(&table);
}

static void test_map_callback_balance(void) {
    LsMapTable table;
    int64_t key;
    int64_t value;
    int64_t out_old = 0;

    g_map_key_copies = 0;
    g_map_key_drops = 0;
    g_map_value_copies = 0;
    g_map_value_drops = 0;

    map_init_i64_i64(&table);
    key = 1;
    value = 10;
    CHECK(!ls_map_insert(&table, &key, &value, &out_old));
    CHECK(g_map_key_copies == 0);
    CHECK(g_map_key_drops == 0);
    CHECK(g_map_value_copies == 0);
    CHECK(g_map_value_drops == 0);

    value = 20;
    CHECK(ls_map_insert(&table, &key, &value, &out_old));
    CHECK(out_old == 10);
    CHECK(g_map_key_drops == 1);
    CHECK(g_map_value_copies == 1);
    CHECK(g_map_value_drops == 1);

    value = 21;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    CHECK(g_map_key_drops == 2);
    CHECK(g_map_value_copies == 1);
    CHECK(g_map_value_drops == 2);

    CHECK(ls_map_remove(&table, &key, &out_old));
    CHECK(out_old == 21);
    CHECK(g_map_key_drops == 3);
    CHECK(g_map_value_drops == 2);

    key = 2;
    value = 30;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    CHECK(ls_map_remove(&table, &key, NULL));
    CHECK(g_map_key_drops == 4);
    CHECK(g_map_value_drops == 3);

    key = 3;
    value = 40;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    key = 4;
    value = 50;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    ls_map_clear(&table);
    CHECK(g_map_key_drops == 6);
    CHECK(g_map_value_drops == 5);
    CHECK(g_map_value_copies == 1);
    CHECK(g_map_key_copies == 0);
    map_free_arrays(&table);
}

/* ---------------------------------------------------------------------- */
/* Hash map stress and adversarial-state tests                            */
/* ---------------------------------------------------------------------- */

/* The control-byte constants the map tests use (MAP_CTRL_*) are declared
 * next to the map test helpers, above map_capacity_ok(). */

/* Stress sizes: compile-time constants keep the aggregate check count small;
 * each test reports a few summary CHECKs instead of one per operation. */
#define MAP_STRESS_I64_KEYS ((size_t)100000)
#define MAP_STRESS_STRING_KEYS ((size_t)20000)
#define MAP_STRESS_STRING_REMOVE_DIVISOR ((size_t)4)
#define MAP_STRESS_GROW_CYCLES ((size_t)3)
#define MAP_STRESS_GROW_KEYS ((size_t)5000)

static int64_t s_map_stress_keys[MAP_STRESS_I64_KEYS];

static void stress_shuffle_i64(int64_t* values, size_t count) {
    size_t i;

    for (i = count; i > 1; i -= 1) {
        size_t j = (size_t)(stress_rng_next() % (uint64_t)i);
        int64_t tmp = values[i - 1];
        values[i - 1] = values[j];
        values[j] = tmp;
    }
}

/* Build deterministic String keys: "<prefix><index padded to 6 digits>". */
static void map_stress_key(char* buffer, size_t size, const char* prefix,
                           size_t index) {
    (void)snprintf(buffer, size, "%s%06zu", prefix, index);
}

/* Compare String-keyed tables in slot iteration order. */
static bool map_string_sequences_equal(const LsMapTable* a,
                                       const LsMapTable* b) {
    LsMapIter ia;
    LsMapIter ib;

    ls_map_iter_init(&ia, a);
    ls_map_iter_init(&ib, b);
    for (;;) {
        const void* key_a = NULL;
        const void* value_a = NULL;
        const void* key_b = NULL;
        const void* value_b = NULL;
        LsString* string_a;
        LsString* string_b;
        bool more_a = ls_map_iter_next(&ia, &key_a, &value_a);
        bool more_b = ls_map_iter_next(&ib, &key_b, &value_b);

        if (more_a != more_b) {
            return false;
        }
        if (!more_a) {
            return true;
        }
        memcpy(&string_a, key_a, sizeof string_a);
        memcpy(&string_b, key_b, sizeof string_b);
        if (!ls_str_eq(string_a, string_b)) {
            return false;
        }
        if (*(const int64_t*)value_a != *(const int64_t*)value_b) {
            return false;
        }
    }
}

/* 100k i64 inserts, lookups, removals and reinserts; iteration sums; equal
 * operation sequences must iterate identically; repeated clear/refill grow
 * cycles must not leak (the table itself owns only three malloc'd arrays). */
static void test_map_stress_i64(void) {
    LsMapTable table;
    LsMapTable twin;
    size_t base = ls_rt_test_live_allocations();
    size_t i;
    size_t found;
    size_t misses;
    size_t removed;
    size_t unexpected;
    size_t cycle;
    size_t count;
    int64_t key_sum;
    int64_t value_sum;
    int64_t expected_key_sum;
    int64_t expected_value_sum;
    int64_t removed_key_sum;
    int64_t removed_value_sum;
    int64_t out_value;
    void* slot = NULL;

    expected_key_sum =
        ((int64_t)MAP_STRESS_I64_KEYS * ((int64_t)MAP_STRESS_I64_KEYS - 1)) /
        2;
    expected_value_sum = expected_key_sum * 3 + (int64_t)MAP_STRESS_I64_KEYS;

    stress_rng_reset();
    for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
        s_map_stress_keys[i] = (int64_t)i;
    }
    stress_shuffle_i64(s_map_stress_keys, MAP_STRESS_I64_KEYS);

    map_init_i64_i64(&table);
    unexpected = 0;
    for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
        int64_t key = s_map_stress_keys[i];
        int64_t value = key * 3 + 1;
        if (ls_map_insert(&table, &key, &value, NULL)) {
            unexpected += 1;
        }
    }
    CHECK(unexpected == 0);
    CHECK(table.len == MAP_STRESS_I64_KEYS);
    CHECK(map_capacity_ok(&table));

    found = 0;
    misses = 0;
    key_sum = 0;
    value_sum = 0;
    for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
        int64_t key = s_map_stress_keys[i];
        slot = NULL;
        if (ls_map_get_ptr(&table, &key, &slot) && slot != NULL) {
            found += 1;
            key_sum += key;
            value_sum += *(const int64_t*)slot;
        } else {
            misses += 1;
        }
    }
    CHECK(found == MAP_STRESS_I64_KEYS);
    CHECK(misses == 0);
    CHECK(key_sum == expected_key_sum);
    CHECK(value_sum == expected_value_sum);
    misses = 0;
    for (i = 0; i < 1000; i += 1) {
        int64_t key = (int64_t)MAP_STRESS_I64_KEYS + (int64_t)i;
        slot = NULL;
        if (ls_map_get_ptr(&table, &key, &slot)) {
            misses += 1;
        }
    }
    CHECK(misses == 0);

    /* A table built by the same operation sequence iterates identically. */
    map_init_i64_i64(&twin);
    unexpected = 0;
    for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
        int64_t key = s_map_stress_keys[i];
        int64_t value = key * 3 + 1;
        if (ls_map_insert(&twin, &key, &value, NULL)) {
            unexpected += 1;
        }
    }
    CHECK(unexpected == 0);
    CHECK(map_sequences_equal(&table, &twin));
    map_free_arrays(&twin);

    removed = 0;
    removed_key_sum = 0;
    removed_value_sum = 0;
    for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
        int64_t key = (int64_t)i;
        if ((i % 3) != 0) {
            continue;
        }
        out_value = -1;
        if (ls_map_remove(&table, &key, &out_value)) {
            removed += 1;
            removed_key_sum += key;
            removed_value_sum += out_value;
        }
    }
    CHECK(removed == (MAP_STRESS_I64_KEYS + 2) / 3);
    CHECK(table.len == MAP_STRESS_I64_KEYS - removed);
    CHECK(map_capacity_ok(&table));
    count = 0;
    key_sum = 0;
    value_sum = 0;
    {
        LsMapIter iter;
        ls_map_iter_init(&iter, &table);
        for (;;) {
            const void* key_ptr = NULL;
            const void* value_ptr = NULL;
            if (!ls_map_iter_next(&iter, &key_ptr, &value_ptr)) {
                break;
            }
            count += 1;
            key_sum += *(const int64_t*)key_ptr;
            value_sum += *(const int64_t*)value_ptr;
        }
    }
    CHECK(count == MAP_STRESS_I64_KEYS - removed);
    CHECK(key_sum == expected_key_sum - removed_key_sum);
    CHECK(value_sum == expected_value_sum - removed_value_sum);

    unexpected = 0;
    for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
        int64_t key = (int64_t)i;
        int64_t value;
        if ((i % 3) != 0) {
            continue;
        }
        value = key * 3 + 2;
        if (ls_map_insert(&table, &key, &value, NULL)) {
            unexpected += 1;
        }
    }
    CHECK(unexpected == 0);
    CHECK(table.len == MAP_STRESS_I64_KEYS);
    found = 0;
    for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
        int64_t key = (int64_t)i;
        int64_t expected = (i % 3) == 0 ? key * 3 + 2 : key * 3 + 1;
        slot = NULL;
        if (ls_map_get_ptr(&table, &key, &slot) && slot != NULL &&
            *(const int64_t*)slot == expected) {
            found += 1;
        }
    }
    CHECK(found == MAP_STRESS_I64_KEYS);

    for (cycle = 0; cycle < MAP_STRESS_GROW_CYCLES; cycle += 1) {
        ls_map_clear(&table);
        CHECK(table.len == 0);
        CHECK(table.cap > 0); /* clear() releases entries, keeps capacity */
        unexpected = 0;
        for (i = 0; i < MAP_STRESS_I64_KEYS; i += 1) {
            int64_t key = s_map_stress_keys[i];
            int64_t value = key + (int64_t)cycle;
            if (ls_map_insert(&table, &key, &value, NULL)) {
                unexpected += 1;
            }
        }
        CHECK(unexpected == 0);
        CHECK(table.len == MAP_STRESS_I64_KEYS);
        CHECK(map_capacity_ok(&table));
    }
    ls_map_clear(&table);
    CHECK(ls_map_is_empty(&table));
    CHECK(ls_rt_test_live_allocations() == base);
    map_free_arrays(&table);
    CHECK(ls_rt_test_live_allocations() == base);
}

/* 20k String keys: content hashing/equality, removals, reinserts, determinism
 * between identical maps, and clear/grow cycles that must release every
 * adopted key handle (checked through the ORC live-allocation counter). */
static void test_map_stress_string_keys(void) {
    LsMapTable table;
    LsMapTable twin;
    LsMapIter iter;
    size_t base = ls_rt_test_live_allocations();
    size_t i;
    size_t cycle;
    size_t found;
    size_t unexpected;
    size_t removed;
    size_t count;
    int64_t value_sum;
    int64_t expected_value_sum;
    int64_t out_value;
    uint64_t key_checksum;
    uint64_t expected_key_checksum;
    char buffer[64];

    expected_value_sum = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        expected_value_sum += (int64_t)(i * 7 + 3);
    }

    map_init_string_i64(&table);
    unexpected = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        LsString* key;
        int64_t value = (int64_t)(i * 7 + 3);
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        key = ls_str_cstr(buffer);
        if (ls_map_insert(&table, &key, &value, NULL)) {
            unexpected += 1;
        }
    }
    CHECK(unexpected == 0);
    CHECK(table.len == MAP_STRESS_STRING_KEYS);
    CHECK(ls_rt_test_live_allocations() ==
          base + MAP_STRESS_STRING_KEYS);

    /* The same operation sequence must produce the same slot iteration. */
    map_init_string_i64(&twin);
    unexpected = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        LsString* key;
        int64_t value = (int64_t)(i * 7 + 3);
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        key = ls_str_cstr(buffer);
        if (ls_map_insert(&twin, &key, &value, NULL)) {
            unexpected += 1;
        }
    }
    CHECK(unexpected == 0);
    CHECK(map_string_sequences_equal(&table, &twin));
    CHECK(ls_rt_test_live_allocations() ==
          base + 2 * MAP_STRESS_STRING_KEYS);
    ls_map_clear(&twin);
    CHECK(ls_rt_test_live_allocations() ==
          base + MAP_STRESS_STRING_KEYS);
    map_free_arrays(&twin);

    found = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        LsString* probe;
        void* slot = NULL;
        int64_t expected = (int64_t)(i * 7 + 3);
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        probe = ls_str_cstr(buffer);
        if (ls_map_get_ptr(&table, &probe, &slot) && slot != NULL &&
            *(const int64_t*)slot == expected) {
            found += 1;
        }
        ls_release(probe);
    }
    CHECK(found == MAP_STRESS_STRING_KEYS);

    count = 0;
    value_sum = 0;
    key_checksum = 0;
    ls_map_iter_init(&iter, &table);
    for (;;) {
        const void* key_ptr = NULL;
        const void* value_ptr = NULL;
        LsString* key;
        if (!ls_map_iter_next(&iter, &key_ptr, &value_ptr)) {
            break;
        }
        memcpy(&key, key_ptr, sizeof key);
        count += 1;
        value_sum += *(const int64_t*)value_ptr;
        key_checksum += ls_hash_bytes(ls_str_data(key), ls_str_len(key));
    }
    CHECK(count == MAP_STRESS_STRING_KEYS);
    CHECK(value_sum == expected_value_sum);
    expected_key_checksum = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        expected_key_checksum += ls_hash_bytes(buffer, strlen(buffer));
    }
    CHECK(key_checksum == expected_key_checksum);

    removed = 0;
    unexpected = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        LsString* probe;
        int64_t expected;
        if ((i % MAP_STRESS_STRING_REMOVE_DIVISOR) != 0) {
            continue;
        }
        expected = (int64_t)(i * 7 + 3);
        out_value = -1;
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        probe = ls_str_cstr(buffer);
        if (ls_map_remove(&table, &probe, &out_value)) {
            removed += 1;
            if (out_value != expected) {
                unexpected += 1;
            }
        }
        ls_release(probe);
    }
    CHECK(removed ==
          MAP_STRESS_STRING_KEYS / MAP_STRESS_STRING_REMOVE_DIVISOR);
    CHECK(unexpected == 0);
    CHECK(table.len == MAP_STRESS_STRING_KEYS - removed);
    CHECK(ls_rt_test_live_allocations() ==
          base + MAP_STRESS_STRING_KEYS - removed);

    found = 0;
    unexpected = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        LsString* probe;
        void* slot = NULL;
        bool present;
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        probe = ls_str_cstr(buffer);
        present = ls_map_get_ptr(&table, &probe, &slot);
        if ((i % MAP_STRESS_STRING_REMOVE_DIVISOR) == 0) {
            if (present) {
                unexpected += 1;
            }
        } else if (present && slot != NULL &&
                   *(const int64_t*)slot == (int64_t)(i * 7 + 3)) {
            found += 1;
        } else {
            unexpected += 1;
        }
        ls_release(probe);
    }
    CHECK(found == MAP_STRESS_STRING_KEYS - removed);
    CHECK(unexpected == 0);

    unexpected = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        LsString* key;
        int64_t value;
        if ((i % MAP_STRESS_STRING_REMOVE_DIVISOR) != 0) {
            continue;
        }
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        key = ls_str_cstr(buffer);
        value = (int64_t)(i * 7 + 1003);
        if (ls_map_insert(&table, &key, &value, NULL)) {
            unexpected += 1;
        }
    }
    CHECK(unexpected == 0);
    CHECK(table.len == MAP_STRESS_STRING_KEYS);
    CHECK(ls_rt_test_live_allocations() ==
          base + MAP_STRESS_STRING_KEYS);
    found = 0;
    for (i = 0; i < MAP_STRESS_STRING_KEYS; i += 1) {
        LsString* probe;
        void* slot = NULL;
        int64_t expected =
            (i % MAP_STRESS_STRING_REMOVE_DIVISOR) == 0
                ? (int64_t)(i * 7 + 1003)
                : (int64_t)(i * 7 + 3);
        map_stress_key(buffer, sizeof buffer, "stress-key-", i);
        probe = ls_str_cstr(buffer);
        if (ls_map_get_ptr(&table, &probe, &slot) && slot != NULL &&
            *(const int64_t*)slot == expected) {
            found += 1;
        }
        ls_release(probe);
    }
    CHECK(found == MAP_STRESS_STRING_KEYS);

    ls_map_clear(&table);
    CHECK(table.len == 0);
    CHECK(ls_rt_test_live_allocations() == base);
    for (cycle = 0; cycle < MAP_STRESS_GROW_CYCLES; cycle += 1) {
        unexpected = 0;
        for (i = 0; i < MAP_STRESS_GROW_KEYS; i += 1) {
            LsString* key;
            int64_t value = (int64_t)(i + cycle * MAP_STRESS_GROW_KEYS);
            map_stress_key(buffer, sizeof buffer, "cycle-key-", i);
            key = ls_str_cstr(buffer);
            if (ls_map_insert(&table, &key, &value, NULL)) {
                unexpected += 1;
            }
        }
        CHECK(unexpected == 0);
        CHECK(table.len == MAP_STRESS_GROW_KEYS);
        CHECK(ls_rt_test_live_allocations() == base + MAP_STRESS_GROW_KEYS);
        unexpected = 0;
        for (i = 0; i < MAP_STRESS_GROW_KEYS; i += 1) {
            LsString* probe;
            if ((i % 2) != 0) {
                continue;
            }
            map_stress_key(buffer, sizeof buffer, "cycle-key-", i);
            probe = ls_str_cstr(buffer);
            if (!ls_map_remove(&table, &probe, NULL)) {
                unexpected += 1;
            }
            ls_release(probe);
        }
        CHECK(unexpected == 0);
        CHECK(table.len == MAP_STRESS_GROW_KEYS / 2);
        ls_map_clear(&table);
        CHECK(table.len == 0);
        CHECK(ls_rt_test_live_allocations() == base);
    }
    map_free_arrays(&table);
    CHECK(ls_rt_test_live_allocations() == base);
}

/* Adversarial table states. The struct is public, so the tests can forge the
 * two defensive probe outcomes that the load-factor invariant otherwise makes
 * unreachable:
 *   1. a full table with no EMPTY slot (growth_left forced to zero): the probe
 *      finds no slot at all, so insert has to rehash before inserting;
 *   2. a table whose first control group is all tombstones while a later group
 *      still has EMPTY slots: the probe wraps around, returns the earliest
 *      tombstone and the insert reuses it without consuming growth_left.
 * Both paths must stay in bounds and keep every existing entry addressable. */
static void test_map_adversarial_states(void) {
    LsMapTable table;
    size_t i;
    size_t slot;
    size_t forged;
    int64_t key;
    int64_t value;
    void* value_ptr = NULL;

    /* (1) Full table without EMPTY slots: insert rehashes and grows. */
    map_init_i64_i64(&table);
    for (i = 0; i < 12; i += 1) {
        key = (int64_t)i;
        value = (int64_t)i * 10;
        CHECK(!ls_map_insert(&table, &key, &value, NULL));
    }
    CHECK(table.cap == 16);
    CHECK(table.len == 12);
    CHECK(table.growth_left == 2);
    CHECK(map_capacity_ok(&table));
    slot = 0;
    forged = 0;
    while (forged < 2) {
        if (table.ctrl[slot] == MAP_CTRL_EMPTY_BYTE) {
            key = 1000 + (int64_t)forged;
            value = 9000 + (int64_t)forged;
            memcpy(table.keys + slot * sizeof(int64_t), &key, sizeof key);
            memcpy(table.values + slot * sizeof(int64_t), &value, sizeof value);
            table.ctrl[slot] =
                (unsigned char)(ls_hash_i64(&key) & MAP_CTRL_H2_LIMIT);
            table.ctrl[table.cap + slot] = table.ctrl[slot];
            table.len += 1;
            table.growth_left -= 1;
            forged += 1;
        }
        slot += 1;
    }
    CHECK(table.len == 14);
    CHECK(table.growth_left == 0);
    CHECK(map_capacity_ok(&table));
    key = 500;
    value = 5000;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    CHECK(table.cap == 32);
    CHECK(table.len == 15);
    CHECK(map_capacity_ok(&table));
    value_ptr = NULL;
    CHECK(ls_map_get_ptr(&table, &key, &value_ptr));
    CHECK(value_ptr != NULL && *(const int64_t*)value_ptr == 5000);
    forged = 0;
    for (i = 0; i < 12; i += 1) {
        int64_t existing = (int64_t)i;
        value_ptr = NULL;
        if (ls_map_get_ptr(&table, &existing, &value_ptr) &&
            value_ptr != NULL &&
            *(const int64_t*)value_ptr == (int64_t)i * 10) {
            forged += 1;
        }
    }
    CHECK(forged == 12);
    for (i = 0; i < 2; i += 1) {
        int64_t existing = 1000 + (int64_t)i;
        value_ptr = NULL;
        CHECK(ls_map_get_ptr(&table, &existing, &value_ptr));
        CHECK(value_ptr != NULL &&
              *(const int64_t*)value_ptr == 9000 + (int64_t)i);
    }
    ls_map_clear(&table);
    CHECK(table.len == 0);
    map_free_arrays(&table);

    /* (2) First control group all tombstones: probe wraps to the next group and
     * reuses the earliest tombstone instead of growing. */
    map_init_i64_i64(&table);
    for (i = 0; i < 15; i += 1) {
        key = (int64_t)i;
        value = (int64_t)i * 10;
        CHECK(!ls_map_insert(&table, &key, &value, NULL));
    }
    CHECK(table.cap == 32);
    ls_map_clear(&table);
    CHECK(table.len == 0);
    CHECK(table.cap == 32);
    for (i = 0; i < MAP_CTRL_GROUP_WIDTH; i += 1) {
        table.ctrl[i] = MAP_CTRL_DELETED_BYTE;
        table.ctrl[table.cap + i] = MAP_CTRL_DELETED_BYTE;
    }
    {
        size_t empty = 0;
        for (i = MAP_CTRL_GROUP_WIDTH; i < table.cap; i += 1) {
            if (table.ctrl[i] == MAP_CTRL_EMPTY_BYTE) {
                empty += 1;
            }
        }
        CHECK(empty == table.cap - MAP_CTRL_GROUP_WIDTH);
        table.growth_left = empty - table.cap / MAP_CTRL_LOAD_DENOMINATOR;
    }
    CHECK(table.growth_left == 12);
    CHECK(map_capacity_ok(&table));
    forged = 0;
    for (key = 0; key < 1000; key += 1) {
        if (((ls_hash_i64(&key) >> 7) & 1u) == 0u) {
            forged = 1;
            break;
        }
    }
    CHECK(forged == 1);
    CHECK(key < 1000);
    value = 5000;
    CHECK(!ls_map_insert(&table, &key, &value, NULL));
    CHECK(table.cap == 32);
    CHECK(table.len == 1);
    CHECK(table.growth_left == 12);
    CHECK(table.ctrl[0] <= MAP_CTRL_H2_LIMIT);
    CHECK(map_capacity_ok(&table));
    value_ptr = NULL;
    CHECK(ls_map_get_ptr(&table, &key, &value_ptr));
    CHECK(value_ptr != NULL && *(const int64_t*)value_ptr == 5000);
    ls_map_clear(&table);
    CHECK(table.len == 0);
    map_free_arrays(&table);
}

/* ====================================================================== */
/* v1.6 Unicode, char and shortest-float regressions                      */
/* ====================================================================== */

static void test_utf8_validation(void) {
    static const unsigned char latin1[] = {'h', 0xC3, 0xA9, 'l', 'l', 'o'};
    static const unsigned char euro[] = {0xE2, 0x82, 0xAC};
    static const unsigned char emoji[] = {0xF0, 0x9F, 0x98, 0x80};
    static const unsigned char max_scalar[] = {0xF4, 0x8F, 0xBF, 0xBF};
    static const unsigned char embedded_nul[] = {'a', 0x00, 'b'};
    static const unsigned char stray_continuation[] = {0x80};
    static const unsigned char overlong_two[] = {0xC0, 0xAF};
    static const unsigned char overlong_three[] = {0xE0, 0x9F, 0xBF};
    static const unsigned char surrogate[] = {0xED, 0xA0, 0x80};
    static const unsigned char above_max[] = {0xF4, 0x90, 0x80, 0x80};
    static const unsigned char five_byte[] = {0xF5, 0x80, 0x80, 0x80};
    static const unsigned char truncated[] = {0xE2, 0x82};
    static const unsigned char bad_continuation[] = {0xC3, 0x28};
    LsString* scalar;

    CHECK(ls_rt_utf8_validate(NULL, 0));
    CHECK(ls_rt_utf8_validate("", 0));
    CHECK(ls_rt_utf8_validate("ascii text", 10));
    CHECK(ls_rt_utf8_validate((const char*)latin1, sizeof latin1));
    CHECK(ls_rt_utf8_validate((const char*)euro, sizeof euro));
    CHECK(ls_rt_utf8_validate((const char*)emoji, sizeof emoji));
    CHECK(ls_rt_utf8_validate((const char*)max_scalar, sizeof max_scalar));
    CHECK(ls_rt_utf8_validate((const char*)embedded_nul, sizeof embedded_nul));
    CHECK(!ls_rt_utf8_validate((const char*)stray_continuation,
                               sizeof stray_continuation));
    CHECK(!ls_rt_utf8_validate((const char*)overlong_two,
                               sizeof overlong_two));
    CHECK(!ls_rt_utf8_validate((const char*)overlong_three,
                               sizeof overlong_three));
    CHECK(!ls_rt_utf8_validate((const char*)surrogate, sizeof surrogate));
    CHECK(!ls_rt_utf8_validate((const char*)above_max, sizeof above_max));
    CHECK(!ls_rt_utf8_validate((const char*)five_byte, sizeof five_byte));
    CHECK(!ls_rt_utf8_validate((const char*)truncated, sizeof truncated));
    CHECK(!ls_rt_utf8_validate((const char*)bad_continuation,
                               sizeof bad_continuation));
    CHECK(ls_rt_utf8_validate("abc", 2)); /* len 2 of an ASCII string: still valid */

    /* Strings built by the runtime are always valid UTF-8. */
    scalar = ls_str_from_scalar(0x1F600u);
    CHECK(ls_rt_utf8_validate(ls_str_data(scalar), ls_str_len(scalar)));
    ls_release(scalar);
    scalar = ls_str_from_parts((const char*)latin1, sizeof latin1);
    CHECK(ls_rt_utf8_validate(ls_str_data(scalar), ls_str_len(scalar)));
    ls_release(scalar);
}

static LsString* hello_from_parts(void) {
    /* "hello" with U+00E9 (two UTF-8 bytes) in place of the first e:
     * 6 bytes, 5 Unicode scalars. */
    static const unsigned char bytes[] = {'h', 0xC3, 0xA9, 'l', 'l', 'o'};
    return ls_str_from_parts((const char*)bytes, sizeof bytes);
}

static void test_char_api(void) {
    LsString* text = hello_from_parts();
    LsString* scalar;
    LsVec_char chars;
    uint32_t value = 0;
    int64_t index;

    CHECK(ls_str_len(text) == 6);
    CHECK(ls_str_char_count(text) == 5);
    CHECK(ls_str_is_char_boundary(text, 0));
    CHECK(!ls_str_is_char_boundary(text, 2)); /* inside the two-byte scalar */
    CHECK(ls_str_is_char_boundary(text, 3));
    CHECK(ls_str_is_char_boundary(text, 6));
    CHECK(!ls_str_is_char_boundary(text, -1));
    CHECK(!ls_str_is_char_boundary(text, 7));
    CHECK(ls_str_char_at(text, 0, &value) && value == 'h');
    CHECK(ls_str_char_at(text, 1, &value) && value == 0xE9u);
    CHECK(ls_str_char_at(text, 4, &value) && value == 'o');
    CHECK(!ls_str_char_at(text, 5, &value));
    CHECK(!ls_str_char_at(text, -1, &value));

    chars = ls_str_chars(text);
    CHECK(chars.len == 5);
    CHECK(chars.data != NULL);
    if (chars.data != NULL) {
        CHECK(chars.data[0] == 'h');
        CHECK(chars.data[1] == 0xE9u);
        CHECK(chars.data[4] == 'o');
    }
    ls_vec_free_bytes(chars.data);

    scalar = ls_str_from_scalar(0x20ACu); /* euro sign */
    CHECK(ls_str_len(scalar) == 3);
    CHECK_STR(scalar, "\xE2\x82\xAC");
    ls_release(scalar);

    /* push/pop operate on scalars and detach shared buffers (COW). */
    scalar = ls_str_cstr("a");
    {
        LsString* shared = (LsString*)ls_retain(scalar);
        bool popped = false;
        uint32_t removed = 0;

        scalar = ls_str_push_scalar(scalar, 0x1F600u);
        CHECK_STR(scalar, "a\xF0\x9F\x98\x80");
        CHECK_STR(shared, "a");
        ls_release(shared);

        scalar = ls_str_pop_scalar(scalar, &popped, &removed);
        CHECK(popped);
        CHECK(removed == 0x1F600u);
        CHECK_STR(scalar, "a");

        scalar = ls_str_pop_scalar(scalar, &popped, &removed);
        CHECK(popped);
        CHECK(removed == 'a');
        CHECK_STR(scalar, "");

        scalar = ls_str_pop_scalar(scalar, &popped, &removed);
        CHECK(!popped);
        CHECK_STR(scalar, "");
    }
    ls_release(scalar);

    /* Every prefix boundary reported by is_char_boundary must be a valid
     * split point. */
    for (index = 0; index <= (int64_t)ls_str_len(text); index += 1) {
        if (ls_str_is_char_boundary(text, index)) {
            LsString* head = ls_str_substring(text, 0, (size_t)index,
                                              TEST_FILE, 0, 0);
            CHECK(head != NULL);
            CHECK(ls_rt_utf8_validate(ls_str_data(head), ls_str_len(head)));
            ls_release(head);
        }
    }
    ls_release(text);
}

static LsString* from_bytes(const unsigned char* bytes, size_t len) {
    return ls_str_from_parts((const char*)bytes, len);
}

static void test_string_unicode(void) {
    /* Full default case mappings. */
    {
        static const unsigned char sharp_s[] = {'a', 0xC3, 0x9F, 'e', 'x'};
        LsString* text = from_bytes(sharp_s, sizeof sharp_s);
        LsString* upper = ls_str_to_uppercase(text);
        CHECK_STR(upper, "ASSEX"); /* U+00DF expands to "SS" */
        ls_release(upper);
        ls_release(text);
    }
    {
        static const unsigned char dotted_i[] = {0xC4, 0xB0}; /* U+0130 */
        LsString* text = from_bytes(dotted_i, sizeof dotted_i);
        LsString* lower = ls_str_to_lowercase(text);
        CHECK_STR(lower, "i\xCC\x87"); /* i + COMBINING DOT ABOVE */
        ls_release(lower);
        ls_release(text);
    }
    {
        static const unsigned char j_caron[] = {0xC7, 0xB0}; /* U+01F0 */
        LsString* text = from_bytes(j_caron, sizeof j_caron);
        LsString* upper = ls_str_to_uppercase(text);
        CHECK_STR(upper, "J\xCC\x8C"); /* J + COMBINING CARON */
        ls_release(upper);
        ls_release(text);
    }

    /* Unicode White_Space trimming. */
    {
        static const unsigned char spaced[] = {0x20, 0xC2, 0xA0, 'x',
                                               0x20, 0xE2, 0x80, 0x83};
        LsString* text = from_bytes(spaced, sizeof spaced);
        LsString* trimmed = ls_str_trim(text);
        CHECK_STR(trimmed, "x"); /* space, NBSP and EM SPACE removed */
        ls_release(trimmed);
        ls_release(text);
    }
    {
        static const unsigned char zero_width[] = {0xE2, 0x80, 0x8B, 'x'};
        LsString* text = from_bytes(zero_width, sizeof zero_width);
        LsString* trimmed = ls_str_trim(text);
        CHECK(trimmed != NULL && ls_str_len(trimmed) == 4);
        CHECK_STR(trimmed, "\xE2\x80\x8Bx"); /* U+200B is not White_Space */
        ls_release(trimmed);
        ls_release(text);
    }

    /* An empty pattern in replace() must iterate scalars, not bytes. */
    {
        LsString* replaced = replace_cstr("h\xC3\xA9llo", "", "-");
        CHECK_STR(replaced, "-h-\xC3\xA9-l-l-o-");
        ls_release(replaced);
    }

    /* An empty separator in split() must also iterate scalars. */
    {
        LsString* text = hello_from_parts();
        LsString* separator = ls_str_cstr("");
        LsVec_String pieces = ls_str_split(text, separator);
        size_t i;

        CHECK(pieces.len == 7);
        if (pieces.len == 7) {
            CHECK_STR(pieces.data[0], "");
            CHECK_STR(pieces.data[1], "h");
            CHECK_STR(pieces.data[2], "\xC3\xA9");
            CHECK_STR(pieces.data[3], "l");
            CHECK_STR(pieces.data[4], "l");
            CHECK_STR(pieces.data[5], "o");
            CHECK_STR(pieces.data[6], "");
        }
        for (i = 0; i < pieces.len; i += 1) {
            CHECK(ls_rt_utf8_validate(ls_str_data(pieces.data[i]),
                                      ls_str_len(pieces.data[i])));
        }
        ls_vec_string_free(&pieces);
        ls_release(separator);
        ls_release(text);
    }

    /* String Debug escaping: printable scalars stay raw, marks are escaped. */
    {
        static const unsigned char mixed[] = {
            'a', 0xC3, 0xA9,             /* a + U+00E9 */
            0xCC, 0x81,                  /* U+0301 COMBINING ACUTE */
            '\t', '"', '\\'};
        LsString* text = from_bytes(mixed, sizeof mixed);
        LsString* rendered = ls_fmt_debug_string(ls_format_begin(0), text);
        CHECK_STR(rendered,
                  "\"a\xC3\xA9" "\\u{301}" "\\t" "\\\"" "\\\\" "\"");
        ls_release(rendered);
        ls_release(text);
    }
    {
        static const unsigned char with_nul[] = {'a', 0x00, 'b'};
        LsString* text = from_bytes(with_nul, sizeof with_nul);
        LsString* rendered = ls_fmt_debug_string(ls_format_begin(0), text);
        CHECK_STR(rendered, "\"a\\u{0}b\"");
        ls_release(rendered);
        ls_release(text);
    }
}

static LsString* render_char(uint32_t v) {
    return ls_fmt_char(ls_format_begin(0), v);
}

static LsString* render_char_debug(uint32_t v) {
    return ls_fmt_char_debug(ls_format_begin(0), v);
}

static void test_char_formatting(void) {
    CHECK_RENDER(render_char('a'), "a");
    CHECK_RENDER(render_char(0xE9u), "\xC3\xA9");
    CHECK_RENDER(render_char_debug('a'), "'a'");
    CHECK_RENDER(render_char_debug(0x27u), "'\\''");
    CHECK_RENDER(render_char_debug('\n'), "'\\n'");
    CHECK_RENDER(render_char_debug('\\'), "'\\\\'");
    CHECK_RENDER(render_char_debug(0x0301u), "'\\u{301}'");
    CHECK_RENDER(render_char_debug(0xE9u), "'\xC3\xA9'");
    CHECK_RENDER(render_char_debug(0x00u), "'\\u{0}'");
}

static void test_format_float_shortest(void) {
    /* Golden values: the shortest representation must match Rust's Display
     * and Debug output. */
    CHECK_RENDER(render_f64(0.0), "0");
    CHECK_RENDER(render_f64_debug(0.0), "0.0");
    CHECK_RENDER(render_f64(1.0), "1");
    CHECK_RENDER(render_f64_debug(1.0), "1.0");
    CHECK_RENDER(render_f64(0.1), "0.1");
    CHECK_RENDER(render_f64_debug(0.1), "0.1");
    CHECK_RENDER(render_f64(1.0 / 3.0), "0.3333333333333333");
    CHECK_RENDER(render_f64_debug(1.0 / 3.0), "0.3333333333333333");
    CHECK_RENDER(render_f64(1e16), "10000000000000000");
    CHECK_RENDER(render_f64_debug(1e16), "1e16");
    CHECK_RENDER(render_f64(1e-5), "0.00001");
    CHECK_RENDER(render_f64_debug(1e-5), "1e-5");
    CHECK_RENDER(render_f64_debug(1e-4), "0.0001");
    CHECK_RENDER(render_f64(-0.0), "-0");
    CHECK_RENDER(render_f64_debug(-0.0), "-0.0");
    CHECK_RENDER(render_f64(INFINITY), "inf");
    CHECK_RENDER(render_f64_debug(INFINITY), "inf");
    CHECK_RENDER(render_f64(-INFINITY), "-inf");
    CHECK_RENDER(render_f64(NAN), "NaN");
    CHECK_RENDER(render_f64_debug(NAN), "NaN");
    CHECK_RENDER(render_f64_debug(5e-324), "5e-324");
    CHECK_RENDER(render_f64_debug(1.7976931348623157e308),
                 "1.7976931348623157e308");
    CHECK_RENDER(render_f64_debug(2.2250738585072014e-308),
                 "2.2250738585072014e-308");
    CHECK_RENDER(render_f64_debug(9007199254740992.0), "9007199254740992.0");
    CHECK_RENDER(render_f64_debug(0.30000000000000004),
                 "0.30000000000000004");
    CHECK_RENDER(render_f32(0.0f), "0");
    CHECK_RENDER(render_f32_debug(0.0f), "0.0");
    CHECK_RENDER(render_f32(1.0f / 3.0f), "0.33333334");
    CHECK_RENDER(render_f32_debug(1.0f / 3.0f), "0.33333334");
    CHECK_RENDER(render_f32_debug(3.4028235e38f), "3.4028235e38");
    CHECK_RENDER(render_f32(INFINITY), "inf");
    CHECK_RENDER(render_f32(NAN), "NaN");
    /* Shortest-round-trip identity on the single-precision boundary: these
     * catch a stale runtime snapshot being embedded in the compiler. */
    CHECK_RENDER(render_f64(16777217.0), "16777217");
    CHECK_RENDER(render_f32(16777216.0f), "16777216");
    CHECK_RENDER(render_f32(16777217.0f), "16777216");
    CHECK_RENDER(render_f32(123456789.0f), "123456790");
}

static void test_str_static(void) {
    static const char literal[] = {'l', 'i', 't', 0x00, 'x'}; /* embedded NUL */
    size_t live_before = ls_rt_test_live_allocations();
    LsString* interned = ls_str_static(literal, sizeof literal);

    CHECK(interned != NULL);
    CHECK(ls_str_len(interned) == sizeof literal);
    CHECK(ls_str_data(interned) != NULL &&
          memcmp(ls_str_data(interned), literal, sizeof literal) == 0);
    /* Immortal allocations are excluded from the live-object accounting. */
    CHECK(ls_rt_test_live_allocations() == live_before);
    ls_release(interned);
    /* Release drops the handle but must never destroy interned bytes. */
    CHECK(ls_str_len(interned) == sizeof literal);
    CHECK(ls_str_data(interned) != NULL &&
          memcmp(ls_str_data(interned), literal, sizeof literal) == 0);
    CHECK(ls_rt_test_live_allocations() == live_before);
}

/* ====================================================================== */
/* Modes                                                                  */
/* ====================================================================== */

static void run_all(const char* io_path) {
    test_alloc();
    test_orc_cycles();
    test_orc_acyclic();
    test_orc_referenced_cycle();
    test_orc_automatic_collection();
    test_orc_live_edge();
    test_orc_stress_many_cycles();
    test_orc_stress_large_cycle();
    test_orc_stress_live_edges();
    test_orc_stress_random_churn();
    test_strings();
    test_cow();
    test_search_transform();
    test_split();
    test_parse();
    test_vec_string();
    test_vec_primitives();
    test_map_hash_primitives();
    test_map_int_basics();
    test_map_iteration_order();
    test_map_growth_iteration();
    test_map_key_types();
    test_map_string_keys();
    test_map_string_values();
    test_map_callback_balance();
    test_map_stress_i64();
    test_map_stress_string_keys();
    test_map_adversarial_states();
    test_utf8_validation();
    test_char_api();
    test_string_unicode();
    test_char_formatting();
    test_format_float_shortest();
    test_str_static();
    test_formatting();
    test_format_specs();
    test_option_result();
    test_checked_math();
    test_io(io_path);
    if (g_failures == 0) {
        printf("ALL CHECKS PASSED (%d checks)\n", g_checks);
    } else {
        printf("CHECKS FAILED: %d of %d\n", g_failures, g_checks);
    }
}

static void mode_args(void) {
    LsVec_String args = ls_args();
    size_t i;
    printf("len=%zu\n", args.len);
    for (i = 0; i < args.len; i += 1) {
        printf("%zu=%s\n", i, ls_str_data(args.data[i]));
    }
    ls_vec_string_free(&args);
}

static void mode_stdin(void) {
    for (;;) {
        LsOpt_String line = ls_read_line();
        if (line.tag == 0) {
            break;
        }
        printf("LINE:%s\n", ls_str_data(line.u.some));
        ls_release(line.u.some);
    }
    printf("EOF\n");
}

static void mode_leak(void) {
    /* Deliberately leaked raw allocations; the leak-tracking builds must
     * report their count and their sizes. */
    void* first = ls_alloc(10);
    void* second = ls_alloc(10);
    (void)first;
    (void)second;
    printf("leaked 10+10 bytes\n");
}

static void mode_orc_shutdown(void) {
    /* Build an unreachable reference cycle and drop both external handles
     * without calling ls_collect_cycles(): shutdown itself must reclaim the
     * cycle, otherwise the leak-tracking builds would report it. */
    OrcTestNode* a = orc_node_new("shutdown-a");
    OrcTestNode* b = orc_node_new("shutdown-b");
    orc_node_set_child(a, b);
    orc_node_set_child(b, a);
    ls_release(a);
    ls_release(b);
    printf("shutdown cycle pending: %zu live allocations\n",
           ls_rt_test_live_allocations());
}

static void mode_leak_managed(void) {
    /* A cycle-capable allocation that is never released and is part of no
     * cycle: reference counting cannot free it and the collector must not
     * collect it, so the leak-tracking builds must report it. */
    (void)ls_alloc_managed(24, orc_test_drop, orc_test_trace);
    printf("leaked 24 managed bytes\n");
}

static int run_panic_mode(const char* mode) {
    if (strcmp(mode, "--panic-div-zero") == 0) {
        (void)ls_div_i64_checked(1, 0, TEST_FILE, 3, 9);
    } else if (strcmp(mode, "--panic-rem-zero") == 0) {
        (void)ls_rem_i64_checked(1, 0, TEST_FILE, 4, 2);
    } else if (strcmp(mode, "--panic-add-overflow") == 0) {
        (void)ls_add_i8_checked(100, 100, TEST_FILE, 5, 1);
    } else if (strcmp(mode, "--panic-mul-overflow") == 0) {
        (void)ls_mul_i64_checked(INT64_MAX, 2, TEST_FILE, 6, 3);
    } else if (strcmp(mode, "--panic-negate-overflow") == 0) {
        (void)ls_neg_i32_checked(INT32_MIN, TEST_FILE, 7, 4);
    } else if (strcmp(mode, "--panic-shift-overflow") == 0) {
        (void)ls_shl_u8_checked(1, 8, TEST_FILE, 8, 5);
    } else if (strcmp(mode, "--panic-div-overflow") == 0) {
        (void)ls_div_i64_checked(INT64_MIN, -1, TEST_FILE, 9, 6);
    } else if (strcmp(mode, "--panic-rem-overflow") == 0) {
        (void)ls_rem_i64_checked(INT64_MIN, -1, TEST_FILE, 10, 7);
    } else if (strcmp(mode, "--panic-bounds") == 0) {
        LsString* s = ls_str_cstr("abc");
        (void)ls_str_substring(s, 99, 0, TEST_FILE, 11, 8);
        ls_release(s);
    } else if (strcmp(mode, "--panic-bounds-boundary") == 0) {
        static const char hello[] = {'h', (char)0xC3, (char)0xA9, 'l', 'l',
                                     'o'};
        LsString* s = ls_str_from_parts(hello, sizeof hello);
        (void)ls_str_substring(s, 2, 1, TEST_FILE, 13, 4);
        ls_release(s);
    } else if (strcmp(mode, "--panic-explicit") == 0) {
        ls_panic_cstr(TEST_FILE, 12, 9, "explicit panic");
    } else if (strcmp(mode, "--panic-double-release") == 0) {
        LsString* s = ls_str_cstr("double");
        ls_release(s);
        ls_release(s); /* the runtime must detect this and exit 101 */
    } else {
        return 0; /* not a panic mode */
    }
    /* All panic paths are noreturn; reaching this point is a failure. */
    printf("UNREACHABLE: panic mode %s returned\n", mode);
    return 1;
}

int main(int argc, char** argv) {
    int result;

    ls_runtime_init(argc, argv);
    if (argc > 1) {
        if (strcmp(argv[1], "--args") == 0) {
            mode_args();
            ls_runtime_shutdown();
            return 0;
        }
        if (strcmp(argv[1], "--stdin") == 0) {
            mode_stdin();
            ls_runtime_shutdown();
            return 0;
        }
        if (strcmp(argv[1], "--leak") == 0) {
            mode_leak();
            ls_runtime_shutdown();
            return 0;
        }
        if (strcmp(argv[1], "--leak-managed") == 0) {
            mode_leak_managed();
            ls_runtime_shutdown();
            return 0;
        }
        if (strcmp(argv[1], "--orc-shutdown") == 0) {
            mode_orc_shutdown();
            ls_runtime_shutdown();
            return 0;
        }
        if (strcmp(argv[1], "--exit-42") == 0) {
            printf("before exit\n");
            ls_exit(42);
        }
        if (strncmp(argv[1], "--panic-", 8) == 0) {
            result = run_panic_mode(argv[1]);
            ls_runtime_shutdown();
            return result;
        }
        if (strcmp(argv[1], "--io") == 0) {
            run_all(argc > 2 ? argv[2] : NULL);
        } else if (strcmp(argv[1], "--io-errors") == 0) {
            test_io_errors(argc > 2 ? argv[2] : NULL);
            if (g_failures == 0) {
                printf("ALL CHECKS PASSED (%d checks)\n", g_checks);
            } else {
                printf("CHECKS FAILED: %d of %d\n", g_failures, g_checks);
            }
            ls_runtime_shutdown();
            return g_failures == 0 ? 0 : 1;
        } else {
            fprintf(stderr, "unknown mode: %s\n", argv[1]);
            ls_runtime_shutdown();
            return 2;
        }
    } else {
        run_all(NULL);
    }
    ls_runtime_shutdown();
    return g_failures == 0 ? 0 : 1;
}
