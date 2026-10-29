/* lsaca C17 runtime: hash map translation unit.
 *
 * Implements the by-value LsMapTable/LsMapIter container that generated code
 * uses for the builtin HashMap<K, V> type. The table is a plain byte container:
 * keys and values are stored with their C sizeof and six callbacks supplied by
 * the code generator provide the language-level hash/equality/copy/drop
 * semantics. Public declarations live in lsaca_rt.h; the shared private
 * contract lives in lsaca_rt_internal.h. The runtime never allocates the
 * generated wrapper, only the three slot arrays.
 *
 * Design:
 *   - Open addressing with linear probing over a power-of-two capacity. The
 *     slot arrays are plain malloc'd buffers owned by the table (they are not
 *     ORC payloads and never appear in leak reports).
 *   - Load factor 3/4: an insert that would take len past 3/4 of the capacity
 *     doubles the capacity (amortized O(1) per insert). Removals mark slots as
 *     tombstones so existing probe sequences stay valid; the next grow rebuilds
 *     the table from the occupied slots only and thereby removes all tombstones.
 *     The frozen struct layout has no tombstone counter, so probe loops are
 *     bounded by cap steps; a probe that has seen every slot without finding an
 *     empty one reuses the first tombstone (len < cap always holds, so a
 *     tombstone must exist then).
 *   - Hashing is deterministic (no randomized seed): FNV-1a 64 over the bytes
 *     followed by the splitmix64 finalizer for byte sequences and strings;
 *     i64/u64/bool hash their 64-bit value (bool as 0/1) through the same
 *     finalizer. Iteration visits slots in index order, so the iteration order
 *     is deterministic for a given sequence of operations and runtime version,
 *     but it is an implementation detail, not part of the language contract.
 *   - get/contains/remove/iteration never allocate; insert allocates only when
 *     the table grows (plus the first insert). Expected complexity: O(1) per
 *     operation, O(cap) only for the rare failed probe, the amortized grow and
 *     iteration/clear.
 *
 * Ownership: an occupied slot owns its key byte blob and its value byte blob
 * (as language-level values, moved in by the caller).
 *   - insert adopts (moves) both incoming blobs. When the key already exists,
 *     the stored value is handed out through copy_value(out_old_value, slot) and
 *     then released, the incoming value is adopted, and the incoming duplicate
 *     key is released through drop_key (ownership of it was transferred to the
 *     map by the caller). The function returns true iff it wrote out_old_value.
 *   - remove moves the stored value bytes into out_value (or releases the stored
 *     value when out_value is NULL) and releases the stored key.
 *   - clear releases every stored key and value but keeps the capacity.
 *   - grow/rehash move occupied key/value bytes with memcpy; this is an
 *     ownership transfer and deliberately does not call any callback.
 * The generated wrapper's drop helper must call ls_map_clear() and then free()
 * keys, values and states; the three pointers are NULL before the first insert.
 *
 * ls_hash_bytes/hashing and the table operations are neither thread safe nor
 * reentrant: callbacks must not mutate the table they are called for.
 */

#include "lsaca_rt_internal.h"

/* ====================================================================== */
/* Hashing and equality primitives                                        */
/* ====================================================================== */

/* Slot state bytes. The values are part of the frozen layout contract. */
#define LS_MAP_EMPTY ((unsigned char)0)
#define LS_MAP_OCCUPIED ((unsigned char)1)
#define LS_MAP_TOMBSTONE ((unsigned char)2)

/* First capacity after the first insert; always a power of two. */
#define LS_MAP_INITIAL_CAPACITY ((size_t)8)

/* Grow when len + 1 would exceed LS_MAP_LOAD_NUMERATOR / LOAD_DENOMINATOR. */
#define LS_MAP_LOAD_NUMERATOR ((size_t)3)
#define LS_MAP_LOAD_DENOMINATOR ((size_t)4)

/* FNV-1a 64-bit parameters (deterministic, frozen for cross-build stability). */
#define LS_MAP_FNV_OFFSET_BASIS ((uint64_t)0xcbf29ce484222325ULL)
#define LS_MAP_FNV_PRIME ((uint64_t)0x100000001b3ULL)

/* splitmix64 finalizer multipliers (deterministic, frozen). */
#define LS_MAP_MIX_MULTIPLY_A ((uint64_t)0xbf58476d1ce4e5b9ULL)
#define LS_MAP_MIX_MULTIPLY_B ((uint64_t)0x94d049bb133111ebULL)

/* splitmix64 finalizer: avalanches a 64-bit value. Applied after FNV-1a so
 * that the low bits used for the slot index are well distributed. */
static uint64_t ls_map_mix64(uint64_t value) {
    value ^= value >> 30;
    value *= LS_MAP_MIX_MULTIPLY_A;
    value ^= value >> 27;
    value *= LS_MAP_MIX_MULTIPLY_B;
    value ^= value >> 31;
    return value;
}

/* FNV-1a over the bytes, then the splitmix64 finalizer. A NULL data pointer
 * with len == 0 (and, defensively, any NULL data pointer) hashes as empty. */
uint64_t ls_hash_bytes(const void* data, size_t len) {
    const unsigned char* bytes = (const unsigned char*)data;
    uint64_t hash = LS_MAP_FNV_OFFSET_BASIS;
    size_t i;

    if (bytes != NULL) {
        for (i = 0; i < len; i += 1) {
            hash ^= (uint64_t)bytes[i];
            hash *= LS_MAP_FNV_PRIME;
        }
    }
    return ls_map_mix64(hash);
}

uint64_t ls_hash_i64(const void* slot) {
    int64_t value;

    memcpy(&value, slot, sizeof value);
    return ls_map_mix64((uint64_t)value);
}

uint64_t ls_hash_u64(const void* slot) {
    uint64_t value;

    memcpy(&value, slot, sizeof value);
    return ls_map_mix64(value);
}

uint64_t ls_hash_bool(const void* slot) {
    bool value;

    memcpy(&value, slot, sizeof value);
    return ls_map_mix64(value ? 1u : 0u);
}

uint64_t ls_hash_string_slot(const void* slot) {
    LsString* value;

    memcpy(&value, slot, sizeof value);
    if (value == NULL) {
        return ls_map_mix64(0);
    }
    return ls_hash_bytes(ls_str_data(value), ls_str_len(value));
}

bool ls_eq_i64(const void* a, const void* b) {
    int64_t left;
    int64_t right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    return left == right;
}

bool ls_eq_u64(const void* a, const void* b) {
    uint64_t left;
    uint64_t right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    return left == right;
}

bool ls_eq_bool(const void* a, const void* b) {
    bool left;
    bool right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    return left == right;
}

bool ls_eq_string_slot(const void* a, const void* b) {
    LsString* left;
    LsString* right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    if (left == NULL || right == NULL) {
        return left == right;
    }
    return ls_str_eq(left, right);
}

/* ====================================================================== */
/* Table internals                                                        */
/* ====================================================================== */

/* Allocate the three slot arrays for the given (power-of-two) capacity.
 * Preconditions, checked defensively: key_size and value_size are non-zero and
 * capacity does not overflow the byte sizes. On allocation failure the process
 * exits through the runtime OOM path. */
static void ls_map_alloc_arrays(LsMapTable* table, size_t capacity) {
    size_t key_bytes;
    size_t value_bytes;

    if (table->key_size == 0 || table->value_size == 0 ||
        capacity > SIZE_MAX / table->key_size ||
        capacity > SIZE_MAX / table->value_size) {
        ls_rt_alloc_failure(SIZE_MAX);
    }
    key_bytes = capacity * table->key_size;
    value_bytes = capacity * table->value_size;
    table->keys = (unsigned char*)malloc(key_bytes);
    if (table->keys == NULL) {
        ls_rt_alloc_failure(key_bytes);
    }
    table->values = (unsigned char*)malloc(value_bytes);
    if (table->values == NULL) {
        ls_rt_alloc_failure(value_bytes);
    }
    table->states = (unsigned char*)calloc(capacity, sizeof(unsigned char));
    if (table->states == NULL) {
        ls_rt_alloc_failure(capacity);
    }
    table->cap = capacity;
}

/* Probe for key. Returns the index of the slot holding it (*found = true) or
 * the index where it should be inserted (*found = false): the first tombstone
 * on the probe path if any, else the first empty slot. The probe is bounded by
 * cap steps. SIZE_MAX means "no usable slot" (cannot happen while the load
 * factor is respected; the caller grows the table and retries defensively). */
static size_t ls_map_find(const LsMapTable* table, const void* key,
                          uint64_t hash, bool* found) {
    size_t mask = table->cap - 1;
    size_t index = (size_t)(hash & (uint64_t)mask);
    size_t first_tombstone = SIZE_MAX;
    size_t probes = 0;

    for (;;) {
        unsigned char state = table->states[index];

        if (state == LS_MAP_EMPTY) {
            *found = false;
            return first_tombstone == SIZE_MAX ? index : first_tombstone;
        }
        if (state == LS_MAP_TOMBSTONE) {
            if (first_tombstone == SIZE_MAX) {
                first_tombstone = index;
            }
        } else if (table->eq_key(table->keys + index * table->key_size, key)) {
            *found = true;
            return index;
        }
        probes += 1;
        if (probes >= table->cap) {
            /* Every slot was probed without meeting an empty one. Since
             * len < cap, a tombstone exists and is a valid insertion slot. */
            *found = false;
            return first_tombstone;
        }
        index = (index + 1) & mask;
    }
}

/* Rebuild the table into a new array pair of the given capacity. Occupied
 * key/value bytes are moved with memcpy (ownership transfer, no callbacks);
 * tombstones are eliminated by construction. */
static void ls_map_rehash(LsMapTable* table, size_t capacity) {
    unsigned char* old_keys = table->keys;
    unsigned char* old_values = table->values;
    unsigned char* old_states = table->states;
    size_t old_cap = table->cap;
    size_t mask = capacity - 1;
    size_t i;

    ls_map_alloc_arrays(table, capacity);
    for (i = 0; i < old_cap; i += 1) {
        size_t index;
        uint64_t hash;

        if (old_states[i] != LS_MAP_OCCUPIED) {
            continue;
        }
        hash = table->hash_key(old_keys + i * table->key_size);
        index = (size_t)(hash & (uint64_t)mask);
        while (table->states[index] != LS_MAP_EMPTY) {
            index = (index + 1) & mask;
        }
        memcpy(table->keys + index * table->key_size,
               old_keys + i * table->key_size, table->key_size);
        memcpy(table->values + index * table->value_size,
               old_values + i * table->value_size, table->value_size);
        table->states[index] = LS_MAP_OCCUPIED;
    }
    free(old_keys);
    free(old_values);
    free(old_states);
}

/* Double the capacity (or create the initial arrays on the first insert). */
static void ls_map_grow(LsMapTable* table) {
    if (table->cap == 0) {
        ls_map_rehash(table, LS_MAP_INITIAL_CAPACITY);
        return;
    }
    if (table->cap > SIZE_MAX / 2) {
        ls_rt_alloc_failure(SIZE_MAX);
    }
    ls_map_rehash(table, table->cap * 2);
}

/* ====================================================================== */
/* Table operations                                                       */
/* ====================================================================== */

void ls_map_init(LsMapTable* table, size_t key_size, size_t value_size,
                 uint64_t (*hash_key)(const void*),
                 bool (*eq_key)(const void*, const void*),
                 void (*copy_key)(void*, const void*),
                 void (*drop_key)(void*),
                 void (*copy_value)(void*, const void*),
                 void (*drop_value)(void*)) {
    memset(table, 0, sizeof *table);
    table->key_size = key_size;
    table->value_size = value_size;
    table->hash_key = hash_key;
    table->eq_key = eq_key;
    table->copy_key = copy_key;
    table->drop_key = drop_key;
    table->copy_value = copy_value;
    table->drop_value = drop_value;
}

bool ls_map_insert(LsMapTable* table, const void* key, const void* value,
                   void* out_old_value) {
    uint64_t hash;
    bool found = false;
    size_t index;

    if (table->cap == 0) {
        ls_map_grow(table);
    }
    hash = table->hash_key(key);
    index = ls_map_find(table, key, hash, &found);
    if (found) {
        unsigned char* slot = table->values + index * table->value_size;

        if (out_old_value != NULL) {
            table->copy_value(out_old_value, slot);
        }
        table->drop_value(slot);
        memcpy(slot, value, table->value_size);
        /* The caller transferred ownership of the key bytes to the map, but the
         * public signature takes them as const (insert never mutates the key).
         * The drop callback only releases/adopts the owned bytes, so the const
         * qualifier is dropped here on purpose. */
        table->drop_key((void*)key);
        return out_old_value != NULL;
    }
    if (index == SIZE_MAX ||
        table->len + 1 >
            (table->cap / LS_MAP_LOAD_DENOMINATOR) * LS_MAP_LOAD_NUMERATOR) {
        ls_map_grow(table);
        index = ls_map_find(table, key, hash, &found);
        /* The key cannot be present in the rebuilt table: only occupied
         * entries are rehashed and the caller's key was not among them. */
    }
    memcpy(table->keys + index * table->key_size, key, table->key_size);
    memcpy(table->values + index * table->value_size, value, table->value_size);
    table->states[index] = LS_MAP_OCCUPIED;
    table->len += 1;
    return false;
}

bool ls_map_get_ptr(const LsMapTable* table, const void* key,
                    void** out_value_ptr) {
    bool found = false;
    size_t index;

    if (table->cap == 0) {
        return false;
    }
    index = ls_map_find(table, key, table->hash_key(key), &found);
    if (!found) {
        return false;
    }
    if (out_value_ptr != NULL) {
        *out_value_ptr = (void*)(table->values + index * table->value_size);
    }
    return true;
}

bool ls_map_remove(LsMapTable* table, const void* key, void* out_value) {
    bool found = false;
    size_t index;

    if (table->cap == 0) {
        return false;
    }
    index = ls_map_find(table, key, table->hash_key(key), &found);
    if (!found) {
        return false;
    }
    if (out_value != NULL) {
        memcpy(out_value, table->values + index * table->value_size,
               table->value_size);
    } else {
        table->drop_value(table->values + index * table->value_size);
    }
    table->drop_key(table->keys + index * table->key_size);
    table->states[index] = LS_MAP_TOMBSTONE;
    table->len -= 1;
    return true;
}

size_t ls_map_len(const LsMapTable* table) {
    return table->len;
}

bool ls_map_is_empty(const LsMapTable* table) {
    return table->len == 0;
}

void ls_map_clear(LsMapTable* table) {
    size_t i;

    if (table->cap == 0) {
        table->len = 0;
        return;
    }
    for (i = 0; i < table->cap; i += 1) {
        unsigned char state = table->states[i];

        if (state == LS_MAP_OCCUPIED) {
            table->states[i] = LS_MAP_EMPTY;
            table->drop_key(table->keys + i * table->key_size);
            table->drop_value(table->values + i * table->value_size);
        } else if (state == LS_MAP_TOMBSTONE) {
            table->states[i] = LS_MAP_EMPTY;
        }
    }
    table->len = 0;
}

void ls_map_iter_init(LsMapIter* iter, const LsMapTable* table) {
    iter->table = table;
    iter->index = 0;
}

bool ls_map_iter_next(LsMapIter* iter, const void** key_ptr,
                      const void** value_ptr) {
    const LsMapTable* table = iter->table;
    size_t index = iter->index;

    while (table != NULL && index < table->cap) {
        if (table->states[index] == LS_MAP_OCCUPIED) {
            if (key_ptr != NULL) {
                *key_ptr = table->keys + index * table->key_size;
            }
            if (value_ptr != NULL) {
                *value_ptr = table->values + index * table->value_size;
            }
            iter->index = index + 1;
            return true;
        }
        index += 1;
    }
    iter->index = index;
    return false;
}
