/* Smoke test for the lsaca C17 runtime.
 *
 * Build (from an ASCII directory, because the repo path has non-ASCII bytes):
 *     gcc -std=c17 -Wall -Wextra -Wpedantic -O1 main.c lsaca_rt.c -lm -o smoke
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
/* Modes                                                                  */
/* ====================================================================== */

static void run_all(const char* io_path) {
    test_alloc();
    test_orc_cycles();
    test_orc_acyclic();
    test_orc_referenced_cycle();
    test_orc_automatic_collection();
    test_orc_live_edge();
    test_strings();
    test_cow();
    test_search_transform();
    test_split();
    test_parse();
    test_vec_string();
    test_vec_primitives();
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
