/* Deterministic randomized ORC / runtime stress harness (v1.6).
 *
 * This program is the ASan/UBSan substitute for the lsaca C runtime: the
 * toolchain used to build the project has neither libasan nor libubsan, so
 * instead we hammer the runtime with randomized mixed workloads and assert
 * that every seed finishes with
 *   - exit code 0,
 *   - empty stderr (no runtime diagnostics, no double release, no panic),
 *   - ls_rt_test_live_allocations() == 0 and no leaked allocation report.
 *
 * Workloads per seed (all deterministic from the seed):
 *   1. randomized managed object graphs (cycles, self edges, extra edges,
 *      retain/release churn, random release order, collector runs at random
 *      points and with a small candidate threshold to force collection while
 *      the graph is partially released);
 *   2. copy-on-write String shares (mutating one handle must not disturb the
 *      other shares or the original);
 *   3. interned literals (ls_str_static) used as owned values: they are
 *      immortal and must not be counted or destroyed by release;
 *   4. HashMap<String, String> insert/overwrite/remove/get/iterate/clear churn
 *      with reference counted keys and values (managed by the map callbacks);
 *   5. a 10k node Box-like chain created, released in random order and
 *      destroyed through the iterative drop cascade.
 *
 * The harness runs every seed in a child process so a crash or a diagnostic
 * cannot hide later seeds. Usage:
 *     orc_random_stress.exe [seed-count]        run seeds (default 32)
 *     orc_random_stress.exe --seed N            internal child mode
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if !defined(_WIN32)
#include <sys/wait.h>
#endif

#include "lsaca_rt.h"

/* Test-only hooks defined in runtime/lsaca_rt.c (not part of the public ABI). */
extern size_t ls_rt_test_live_allocations(void);
extern size_t ls_rt_test_candidate_threshold(void);
extern size_t ls_rt_test_set_candidate_threshold(size_t threshold);

#define RANDOM_STRESS_DEFAULT_SEEDS 32L
#define RANDOM_STRESS_MAX_SEEDS 256L
#define RANDOM_STRESS_POOL_KEYS 12
#define RANDOM_STRESS_MAP_CAPACITY 64
#define RANDOM_STRESS_GRAPH_MIN 20
#define RANDOM_STRESS_GRAPH_SPAN 181
#define RANDOM_STRESS_DEEP_CHAIN 10000
#define RANDOM_STRESS_MAP_OPS 300

static uint64_t g_rng_state;
static uint64_t g_seed;
static const char* g_exe;

#define REQUIRE(expr)                                                          \
    do {                                                                       \
        if (!(expr)) {                                                         \
            fprintf(stderr, "seed %llu: check failed at %s:%d: %s\n",         \
                    (unsigned long long)g_seed, __FILE__, __LINE__, #expr);    \
            return 1;                                                          \
        }                                                                      \
    } while (0)

/* ---------------------------------------------------------------------- */
/* Deterministic PRNG (splitmix64).                                       */
/* ---------------------------------------------------------------------- */

static uint64_t rng_next(void) {
    uint64_t z = (g_rng_state += 0x9E3779B97F4A7C15ULL);
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
    return z ^ (z >> 31);
}

static size_t rng_below(size_t bound) {
    if (bound == 0) {
        return 0;
    }
    return (size_t)(rng_next() % (uint64_t)bound);
}

static void shuffle_sizes(size_t* values, size_t count) {
    size_t index;
    if (count < 2) {
        return;
    }
    for (index = count - 1; index > 0; index -= 1) {
        size_t other = rng_below(index + 1);
        size_t swap = values[index];
        values[index] = values[other];
        values[other] = swap;
    }
}

/* ---------------------------------------------------------------------- */
/* Small allocation helpers used for random text and file capture.        */
/* ---------------------------------------------------------------------- */

static LsString* random_ascii_string(void) {
    char buffer[16];
    size_t length = rng_below(13);
    size_t index;
    for (index = 0; index < length; index += 1) {
        buffer[index] = (char)('a' + (int)rng_below(20));
    }
    return ls_str_from_parts(buffer, length);
}

static LsString* random_key_string(size_t index) {
    char buffer[32];
    int written = snprintf(buffer, sizeof buffer, "key%zu-%c", index,
                           (char)('a' + (int)rng_below(3)));
    if (written < 0) {
        written = 0;
    }
    return ls_str_from_parts(buffer, (size_t)written);
}

typedef struct Bytes {
    char* data;
    size_t len;
} Bytes;

static int read_whole_file(const char* path, Bytes* out) {
    FILE* file = fopen(path, "rb");
    long size;
    size_t read_count;
    out->data = NULL;
    out->len = 0;
    if (file == NULL) {
        return 0;
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        (void)fclose(file);
        return 0;
    }
    size = ftell(file);
    if (size < 0 || fseek(file, 0, SEEK_SET) != 0) {
        (void)fclose(file);
        return 0;
    }
    out->data = (char*)malloc((size_t)size + 1);
    if (out->data == NULL) {
        (void)fclose(file);
        return 0;
    }
    read_count = fread(out->data, 1, (size_t)size, file);
    if (read_count != (size_t)size) {
        free(out->data);
        out->data = NULL;
        (void)fclose(file);
        return 0;
    }
    out->data[read_count] = '\0';
    out->len = read_count;
    if (fclose(file) != 0) {
        free(out->data);
        out->data = NULL;
        out->len = 0;
        return 0;
    }
    return 1;
}

static void bytes_free(Bytes* bytes) {
    free(bytes->data);
    bytes->data = NULL;
    bytes->len = 0;
}

/* ---------------------------------------------------------------------- */
/* Randomized managed object graph.                                       */
/* ---------------------------------------------------------------------- */

typedef struct RandomNode {
    LsString* name;          /* owned managed handle */
    struct RandomNode* child; /* owned managed handle */
    struct RandomNode* extra; /* owned managed handle */
    uint64_t tag;
} RandomNode;

static void random_node_drop(void* payload) {
    RandomNode* node = (RandomNode*)payload;
    LsString* name = node->name;
    RandomNode* child = node->child;
    RandomNode* extra = node->extra;

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
}

static void random_node_trace(const void* payload, LsTraceVisit visit) {
    const RandomNode* node = (const RandomNode*)payload;
    if (node->child != NULL) {
        visit(node->child);
    }
    if (node->extra != NULL) {
        visit(node->extra);
    }
}

static RandomNode* random_node_new(uint64_t tag) {
    RandomNode* node =
        (RandomNode*)ls_alloc_managed(sizeof(RandomNode), random_node_drop,
                                      random_node_trace);
    if (node == NULL) {
        return NULL;
    }
    node->tag = tag;
    return node;
}

static int test_random_graph(void) {
    size_t count = RANDOM_STRESS_GRAPH_MIN + rng_below(RANDOM_STRESS_GRAPH_SPAN);
    RandomNode** nodes = (RandomNode**)malloc(count * sizeof(*nodes));
    RandomNode** held = (RandomNode**)malloc(count * sizeof(*held));
    size_t* order = (size_t*)malloc(count * sizeof(*order));
    size_t held_count = 0;
    size_t index;

    REQUIRE(nodes != NULL && held != NULL && order != NULL);
    for (index = 0; index < count; index += 1) {
        nodes[index] = random_node_new((uint64_t)index);
        REQUIRE(nodes[index] != NULL);
        nodes[index]->name = random_ascii_string();
        REQUIRE(nodes[index]->name != NULL);
    }
    for (index = 0; index < count; index += 1) {
        if (rng_below(100) < 70) {
            RandomNode* target = nodes[rng_below(count)];
            nodes[index]->child = (RandomNode*)ls_retain(target);
        }
        if (rng_below(100) < 20) {
            RandomNode* target = nodes[rng_below(count)];
            nodes[index]->extra = (RandomNode*)ls_retain(target);
        }
    }
    for (index = 0; index < count; index += 1) {
        RandomNode* target = nodes[rng_below(count)];
        RandomNode* handle = (RandomNode*)ls_retain(target);
        if (rng_below(2) == 0) {
            ls_release(handle);
        } else {
            held[held_count] = handle;
            held_count += 1;
        }
    }
    for (index = 0; index < count; index += 1) {
        order[index] = index;
    }
    shuffle_sizes(order, count);
    for (index = 0; index < count; index += 1) {
        if (rng_below(6) == 0) {
            ls_collect_cycles();
        }
        ls_release(nodes[order[index]]);
    }
    /* Release the retained handles in random order. The order array still
     * holds the permutation of node indices used above, so it must be rebuilt
     * as a permutation of held indices before being used as held[] subscripts. */
    for (index = 0; index < held_count; index += 1) {
        order[index] = index;
    }
    shuffle_sizes(order, held_count);
    for (index = 0; index < held_count; index += 1) {
        ls_release(held[order[index]]);
    }
    ls_collect_cycles();
    REQUIRE(ls_rt_test_live_allocations() == 0);
    free(nodes);
    free(held);
    free(order);
    return 0;
}

/* ---------------------------------------------------------------------- */
/* Copy-on-write String shares.                                           */
/* ---------------------------------------------------------------------- */

static int test_cow_strings(void) {
    LsString* base = random_ascii_string();
    LsString* original;
    LsString* shares[4];
    size_t lengths[4];
    size_t base_length;
    size_t index;

    REQUIRE(base != NULL);
    base_length = ls_str_len(base);
    original = ls_str_from_parts(ls_str_data(base), base_length);
    REQUIRE(original != NULL);
    for (index = 0; index < 4; index += 1) {
        shares[index] = (LsString*)ls_retain(base);
        lengths[index] = base_length;
    }
    for (index = 0; index < 4; index += 1) {
        uint64_t mode = rng_next() % 3;
        if (mode == 0) {
            char byte = (char)('a' + (int)rng_below(26));
            shares[index] = ls_str_push_char(shares[index], byte);
            lengths[index] += 1;
        } else if (mode == 1) {
            shares[index] = ls_str_push_cstr(shares[index], "xy");
            lengths[index] += 2;
        } else {
            /* U+00E9 (e acute) encodes as two UTF-8 bytes. */
            shares[index] = ls_str_push_scalar(shares[index], 0x00E9u);
            lengths[index] += 2;
        }
        REQUIRE(shares[index] != NULL);
        REQUIRE(ls_str_len(shares[index]) == lengths[index]);
    }
    REQUIRE(ls_str_len(base) == base_length);
    REQUIRE(ls_str_eq(base, original));
    for (index = 0; index < 4; index += 1) {
        ls_release(shares[index]);
    }
    ls_release(base);
    ls_release(original);
    REQUIRE(ls_rt_test_live_allocations() == 0);
    return 0;
}

/* ---------------------------------------------------------------------- */
/* Interned literals (ls_str_static) used as owned values.                */
/* ---------------------------------------------------------------------- */

static int test_interned_literals(void) {
    char buffer[20];
    size_t length = rng_below(17);
    size_t index;
    size_t base_live = ls_rt_test_live_allocations();
    LsString* literal;
    LsString* copy;
    LsString* retained;

    for (index = 0; index < length; index += 1) {
        buffer[index] = (char)('A' + (int)rng_below(26));
    }
    literal = ls_str_static(buffer, length);
    REQUIRE(literal != NULL);
    /* The interned allocation is immortal and excluded from the live count. */
    REQUIRE(ls_rt_test_live_allocations() == base_live);
    copy = ls_str_from_parts(buffer, length);
    REQUIRE(copy != NULL);
    REQUIRE(ls_rt_test_live_allocations() == base_live + 1);
    REQUIRE(ls_str_eq(literal, copy));
    REQUIRE(ls_str_len(literal) == length);
    retained = (LsString*)ls_retain(literal);
    REQUIRE(ls_refcount(retained) == ls_refcount(literal));
    ls_release(retained);
    /* Releasing the caller handle drops the count but never frees it. */
    ls_release(literal);
    REQUIRE(ls_rt_test_live_allocations() == base_live + 1);
    ls_release(copy);
    REQUIRE(ls_rt_test_live_allocations() == base_live);
    return 0;
}

/* ---------------------------------------------------------------------- */
/* HashMap<String, String>: reference counted keys and values.            */
/* ---------------------------------------------------------------------- */

static uint64_t map_hash_string(const void* key_slot) {
    return ls_hash_string_slot(key_slot);
}

static bool map_eq_string(const void* left, const void* right) {
    return ls_eq_string_slot(left, right);
}

static void map_copy_string(void* out, const void* in) {
    *(void**)out = ls_retain(*(void* const*)in);
}

static void map_drop_string(void* slot) {
    void* value = *(void**)slot;
    if (value != NULL) {
        ls_release(value);
    }
}

static size_t map_model_find(LsString** model_keys, size_t model_len,
                             const LsString* key) {
    size_t index;
    for (index = 0; index < model_len; index += 1) {
        if (ls_str_eq(model_keys[index], key)) {
            return index;
        }
    }
    return model_len;
}

static int map_verify(const LsMapTable* table, LsString** model_keys,
                      LsString** model_values, size_t model_len) {
    LsMapIter iter;
    const void* key_ptr;
    const void* value_ptr;
    size_t iterated = 0;
    size_t index;

    if (ls_map_len(table) != model_len) {
        return 0;
    }
    if (ls_map_is_empty(table) != (model_len == 0)) {
        return 0;
    }
    for (index = 0; index < model_len; index += 1) {
        LsString* probe = model_keys[index];
        void* slot = NULL;
        LsString* stored;
        if (!ls_map_get_ptr(table, &probe, &slot)) {
            return 0;
        }
        stored = *(LsString**)slot;
        if (!ls_str_eq(stored, model_values[index])) {
            return 0;
        }
    }
    /* Every occupied slot must be reachable through the public iterator. */
    ls_map_iter_init(&iter, table);
    while (ls_map_iter_next(&iter, &key_ptr, &value_ptr)) {
        LsString* stored_key = *(LsString* const*)key_ptr;
        LsString* stored_value = *(LsString* const*)value_ptr;
        LsString* probe = stored_key;
        void* slot = NULL;
        (void)stored_value;
        if (!ls_map_get_ptr(table, &probe, &slot)) {
            return 0;
        }
        if (!ls_str_eq(*(LsString**)slot, stored_value)) {
            return 0;
        }
        iterated += 1;
    }
    return iterated == model_len;
}

static int test_map_churn(void) {
    LsMapTable table;
    LsString* pool[RANDOM_STRESS_POOL_KEYS];
    LsString* model_keys[RANDOM_STRESS_MAP_CAPACITY];
    LsString* model_values[RANDOM_STRESS_MAP_CAPACITY];
    size_t model_len = 0;
    size_t index;
    int step;

    memset(&table, 0, sizeof table);
    memset(model_keys, 0, sizeof model_keys);
    memset(model_values, 0, sizeof model_values);
    for (index = 0; index < RANDOM_STRESS_POOL_KEYS; index += 1) {
        pool[index] = random_key_string(index);
        REQUIRE(pool[index] != NULL);
    }
    ls_map_init(&table, sizeof(LsString*), sizeof(LsString*), map_hash_string,
                map_eq_string, map_copy_string, map_drop_string,
                map_copy_string, map_drop_string);

    for (step = 0; step < RANDOM_STRESS_MAP_OPS; step += 1) {
        uint64_t mode = rng_next() % 10;
        if (mode < 6) {
            /* Insert or overwrite. */
            LsString* key = (LsString*)ls_retain(pool[rng_below(RANDOM_STRESS_POOL_KEYS)]);
            LsString* value = (LsString*)ls_retain(pool[rng_below(RANDOM_STRESS_POOL_KEYS)]);
            LsString* old_value = NULL;
            size_t found = map_model_find(model_keys, model_len, key);
            bool had_old;

            REQUIRE(key != NULL && value != NULL);
            had_old = ls_map_insert(&table, &key, &value, &old_value);
            if (found < model_len) {
                LsString* previous = model_values[found];
                model_values[found] = (LsString*)ls_retain(value);
                REQUIRE(model_values[found] != NULL);
                if (previous != NULL) {
                    ls_release(previous);
                }
                REQUIRE(had_old);
                ls_release(old_value);
                REQUIRE(ls_str_eq(model_values[found], value));
            } else {
                REQUIRE(!had_old);
                REQUIRE(old_value == NULL);
                model_keys[model_len] = (LsString*)ls_retain(key);
                model_values[model_len] = (LsString*)ls_retain(value);
                REQUIRE(model_keys[model_len] != NULL);
                REQUIRE(model_values[model_len] != NULL);
                model_len += 1;
            }
        } else if (mode < 8 && model_len != 0) {
            /* Remove a random resident entry. */
            size_t victim = rng_below(model_len);
            LsString* lookup = model_keys[victim];
            LsString* removed = NULL;
            REQUIRE(ls_map_remove(&table, &lookup, &removed));
            REQUIRE(removed != NULL);
            REQUIRE(ls_str_eq(removed, model_values[victim]));
            ls_release(removed);
            ls_release(model_keys[victim]);
            ls_release(model_values[victim]);
            model_keys[victim] = model_keys[model_len - 1];
            model_values[victim] = model_values[model_len - 1];
            model_len -= 1;
        } else if (mode < 9) {
            /* Contains-style probe: present entries hit, absent ones miss. */
            if (rng_below(2) == 0 && model_len != 0) {
                size_t probe_index = rng_below(model_len);
                LsString* probe = model_keys[probe_index];
                void* slot = NULL;
                REQUIRE(ls_map_get_ptr(&table, &probe, &slot));
                REQUIRE(ls_str_eq(*(LsString**)slot, model_values[probe_index]));
            } else if (model_len < RANDOM_STRESS_MAP_CAPACITY) {
                LsString* absent = random_ascii_string();
                void* slot = NULL;
                REQUIRE(absent != NULL);
                REQUIRE(!ls_map_get_ptr(&table, &absent, &slot));
                ls_release(absent);
            }
        } else {
            REQUIRE(map_verify(&table, model_keys, model_values, model_len));
        }
        REQUIRE(map_verify(&table, model_keys, model_values, model_len));
    }

    /* Interned literals stored in the map as owned key/value handles. */
    {
        LsString* static_key = ls_str_static("static-key", 10);
        LsString* static_value = ls_str_static("static-value", 12);
        void* slot = NULL;
        size_t live_bound = ls_rt_test_live_allocations();
        REQUIRE(static_key != NULL && static_value != NULL);
        REQUIRE(ls_map_insert(&table, &static_key, &static_value, NULL) == false);
        REQUIRE(ls_map_get_ptr(&table, &static_key, &slot));
        REQUIRE(ls_str_eq(*(LsString**)slot, static_value));
        /* insert() adopted both interned handles: the map owns them until
         * ls_map_clear(), so the caller must not release them here. */
        /* Map lookups need a handle; build a temporary equivalent key. */
        {
            LsString* probe = ls_str_from_parts("static-key", 10);
            REQUIRE(probe != NULL);
            REQUIRE(ls_map_get_ptr(&table, &probe, &slot));
            ls_release(probe);
        }
        REQUIRE(ls_rt_test_live_allocations() == live_bound); /* probe released */
    }

    ls_map_clear(&table);
    free(table.keys);
    free(table.values);
    free(table.ctrl);
    for (index = 0; index < model_len; index += 1) {
        ls_release(model_keys[index]);
        ls_release(model_values[index]);
    }
    for (index = 0; index < RANDOM_STRESS_POOL_KEYS; index += 1) {
        ls_release(pool[index]);
    }
    REQUIRE(ls_rt_test_live_allocations() == 0);
    return 0;
}

/* ---------------------------------------------------------------------- */
/* Deep 10k-node chain with random release order.                         */
/* ---------------------------------------------------------------------- */

static int test_deep_chain(void) {
    size_t count = RANDOM_STRESS_DEEP_CHAIN;
    size_t previous_threshold =
        ls_rt_test_set_candidate_threshold((size_t)-1);
    RandomNode** nodes = (RandomNode**)malloc(count * sizeof(*nodes));
    size_t* order = (size_t*)malloc(count * sizeof(*order));
    size_t index;

    REQUIRE(nodes != NULL && order != NULL);
    for (index = 0; index < count; index += 1) {
        nodes[index] = random_node_new((uint64_t)index);
        REQUIRE(nodes[index] != NULL);
        nodes[index]->name = random_ascii_string();
        REQUIRE(nodes[index]->name != NULL);
    }
    for (index = 0; index + 1 < count; index += 1) {
        nodes[index]->child = (RandomNode*)ls_retain(nodes[index + 1]);
    }
    REQUIRE(ls_rt_test_live_allocations() == 2 * count);
    for (index = 0; index < count; index += 1) {
        order[index] = index;
    }
    /* Release every array handle except the head in random order; releasing
     * the head last cascades through the whole chain iteratively. */
    order[0] = 0;
    for (index = 1; index < count; index += 1) {
        order[index] = index;
    }
    shuffle_sizes(order + 1, count - 1);
    for (index = 1; index < count; index += 1) {
        ls_release(nodes[order[index]]);
    }
    REQUIRE(ls_rt_test_live_allocations() == 2 * count);
    ls_release(nodes[0]);
    REQUIRE(ls_rt_test_live_allocations() == 0);
    free(nodes);
    free(order);
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
    return 0;
}

/* ---------------------------------------------------------------------- */
/* Per-seed driver and child-process orchestration.                       */
/* ---------------------------------------------------------------------- */

static int run_one_seed(void) {
    size_t previous_threshold;

    g_rng_state = g_seed;
    /* Force collections while graphs are partially released. */
    previous_threshold =
        ls_rt_test_set_candidate_threshold(3 + rng_below(38));
    if (test_random_graph() != 0) {
        return 1;
    }
    if (test_cow_strings() != 0) {
        return 1;
    }
    if (test_interned_literals() != 0) {
        return 1;
    }
    if (test_map_churn() != 0) {
        return 1;
    }
    if (test_deep_chain() != 0) {
        return 1;
    }
    (void)ls_rt_test_set_candidate_threshold(previous_threshold);
    ls_collect_cycles();
    REQUIRE(ls_rt_test_live_allocations() == 0);
    return 0;
}

static int run_child_seed(const char* text, int argc, char** argv) {
    char* end = NULL;
    unsigned long long parsed = strtoull(text, &end, 10);
    int status;

    if (end == NULL || *end != '\0') {
        fprintf(stderr, "invalid seed argument: %s\n", text);
        return 3;
    }
    g_seed = (uint64_t)parsed;
    (void)ls_runtime_init(argc, argv);
    status = run_one_seed();
    if (status == 0) {
        printf("SEED %llu OK live=0\n", (unsigned long long)g_seed);
    }
    ls_runtime_shutdown();
    return status;
}

static int run_seed_child(uint64_t seed) {
    char out_path[64];
    char err_path[64];
    char expected[64];
    char command[4096];
    Bytes out;
    Bytes err;
    int written;
    int status;
    int result = 1;

    (void)snprintf(out_path, sizeof out_path, "orc_random_%llu.out",
                   (unsigned long long)seed);
    (void)snprintf(err_path, sizeof err_path, "orc_random_%llu.err",
                   (unsigned long long)seed);
    written = snprintf(command, sizeof command,
                       "cmd /c \"\"%s\" --seed %llu > \"%s\" 2> \"%s\"\"",
                       g_exe, (unsigned long long)seed, out_path, err_path);
    if (written < 0 || (size_t)written >= sizeof command) {
        fprintf(stderr, "seed %llu: command line too long\n",
                (unsigned long long)seed);
        return 1;
    }
    status = system(command);
#ifndef _WIN32
    if (status != -1) {
        status = WIFEXITED(status) ? WEXITSTATUS(status) : 128;
    }
#endif
    if (!read_whole_file(out_path, &out)) {
        fprintf(stderr, "seed %llu: could not read %s\n",
                (unsigned long long)seed, out_path);
        return 1;
    }
    if (!read_whole_file(err_path, &err)) {
        fprintf(stderr, "seed %llu: could not read %s\n",
                (unsigned long long)seed, err_path);
        bytes_free(&out);
        return 1;
    }
    (void)remove(out_path);
    (void)remove(err_path);
    (void)snprintf(expected, sizeof expected, "SEED %llu OK live=0\n",
                   (unsigned long long)seed);
    if (status != 0) {
        fprintf(stderr, "seed %llu: child exit code %d\n",
                (unsigned long long)seed, status);
    } else if (err.len != 0) {
        fprintf(stderr, "seed %llu: child wrote stderr: %s\n",
                (unsigned long long)seed, err.data);
    } else if (out.len != strlen(expected) ||
               memcmp(out.data, expected, out.len) != 0) {
        fprintf(stderr, "seed %llu: unexpected child stdout: %s\n",
                (unsigned long long)seed, out.data);
    } else {
        result = 0;
    }
    bytes_free(&out);
    bytes_free(&err);
    return result;
}

static int run_parent(int argc, char** argv) {
    long requested = RANDOM_STRESS_DEFAULT_SEEDS;
    long index;
    int failures = 0;

    g_exe = argv[0];
    if (argc >= 2) {
        char* end = NULL;
        long parsed = strtol(argv[1], &end, 10);
        if (end == NULL || *end != '\0' || parsed < 1) {
            fprintf(stderr, "usage: %s [seed-count]\n", g_exe);
            return 2;
        }
        requested = parsed;
    }
    if (requested > RANDOM_STRESS_MAX_SEEDS) {
        requested = RANDOM_STRESS_MAX_SEEDS;
    }
    for (index = 0; index < requested; index += 1) {
        uint64_t seed = 0x9E3779B97F4A7C15ULL * ((uint64_t)index + 1);
        seed ^= 0xD1B54A32D192ED03ULL;
        if (run_seed_child(seed) != 0) {
            failures += 1;
        }
    }
    if (failures == 0) {
        printf("RANDOM STRESS PASSED seeds=%ld\n", requested);
        return 0;
    }
    printf("RANDOM STRESS FAILURES: %d seeds=%ld\n", failures, requested);
    return 1;
}

int main(int argc, char** argv) {
    if (argc >= 3 && strcmp(argv[1], "--seed") == 0) {
        return run_child_seed(argv[2], argc, argv);
    }
    return run_parent(argc, argv);
}
