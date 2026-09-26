#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "src/base/lsaca_map.h"
#include "src/base/lsaca_gc.h"
#include "src/base/lsaca_object.h"

#define GROUP_WIDTH 8
#define EMPTY 0x00
#define TOMB 0x80
#define H2_MASK 0x7F
#define LOAD_FACTOR 0.875

#define ALIGN2(x) (((x) + 2 - 1) & ~(2 - 1))

static void ls_entry_trace(void *obj_ptr, ls_gc_ref_visitor visitor) {
    ls_map_entry_t *entry = (ls_map_entry_t *)obj_ptr;
    switch (LS_GET_TYPE(entry->key)) {
        case LS_ANY_STR: visitor((void **)&LS_AS_STR(entry->key)); break;
        case LS_ANY_FUNC: visitor((void **)&LS_AS_FUNC(entry->key)); break;
        default: break;
    }
    switch (LS_GET_TYPE(entry->value)) {
        case LS_ANY_STR: visitor((void **)&LS_AS_STR(entry->value)); break;
        case LS_ANY_FUNC: visitor((void **)&LS_AS_FUNC(entry->value)); break;
        default: break;
    }
}

LS_GC_TYPE(ls_entry_name, ls_map_entry_t, ls_entry_trace);

static uint64_t hash_any(ls_any_t value) {
    switch (LS_GET_TYPE(value)) {
        case LS_ANY_NULL:
        case LS_ANY_BOOL:
        case LS_ANY_I64:
        case LS_ANY_F64: {
            uint64_t x = LS_AS_I64(value);
            x = (x ^ (x >> 33)) * 0xFF51AFD7ED558CCDULL;
            x = (x ^ (x >> 33)) * 0xC4CEB9FE1A85EC53ULL;
            x = x ^ (x >> 33);
            return x;
        }
        case LS_ANY_STR: {
            uint64_t hash = 14695981039346656037ULL;
            ls_string_t *str = LS_AS_STR(value);
            for (size_t i = 0; i < str->size; ++i) {
                hash ^= (uint64_t)(unsigned char)str->c_str[i];
                hash *= 1099511628211ULL;
            }
            return hash;
        }
        case LS_ANY_FUNC: {
            uint64_t hash = 14695981039346656037ULL;
            ls_function_t *function = LS_AS_FUNC(value);
            hash ^= hash_any(LS_STR_ANY(function->name));
            hash <<= function->arity;
            hash *= 1099511628211ULL;
            hash >>= function->arity;
            return hash;
        }
        default: return 0;
    }
}

static bool is_key_equal(ls_any_t a, ls_any_t b) {
    if (LS_GET_TYPE(a) != LS_GET_TYPE(b)) return false;
    switch (LS_GET_TYPE(a)) {
        case LS_ANY_NULL: {
            return true;
        }
        case LS_ANY_BOOL: {
            return LS_AS_BOOL(a) == LS_AS_BOOL(b);
        }
        case LS_ANY_F64: {
            return fabs(LS_AS_F64(a) - LS_AS_F64(b)) < 1e-12;
        }
        case LS_ANY_I64: {
            return LS_AS_I64(a) == LS_AS_I64(b);
        }
        case LS_ANY_STR: {
            return ls_string_is_equal(LS_AS_STR(a), LS_AS_STR(b));
        }
        case LS_ANY_FUNC: {
            ls_function_t *func_a = LS_AS_FUNC(a);
            ls_function_t *func_b = LS_AS_FUNC(b);
            if (func_a->arity != func_b->arity) return false;
            if (!ls_string_is_equal(func_a->name, func_b->name)) return false;
            if (func_a->chunk.size != func_b->chunk.size) return false;
            if (func_a->chunk.constants.size != func_b->chunk.constants.size) return false;
            for (size_t i = 0; i < func_a->chunk.size; ++i) {
                if (func_a->chunk.codes[i] != func_b->chunk.codes[i]) return false;
                if (func_a->chunk.lines[i] != func_b->chunk.lines[i]) return false;
            }
            for (size_t i = 0; i < func_a->chunk.constants.size; ++i) {
                if (!is_key_equal(func_a->chunk.constants.values[i], func_b->chunk.constants.values[i])) return false;
            }
            return true;
        }
        default: return false;
    }
}

static uint8_t h2_from_hash(uint64_t hash) {
    uint8_t h2 = (uint8_t)(hash >> 57);
    return (h2 == 0) ? 1 : (h2 & H2_MASK);
}

static uint8_t match_h2(const uint8_t *group, uint8_t h2) {
    uint8_t mask = 0;
    for (size_t i = 0; i < GROUP_WIDTH; ++i) {
        if ((group[i] & H2_MASK) == h2) {
            mask |= (1u << i);
        }
    }
    return mask;
}

static uint8_t match_empty(const uint8_t *group) {
    uint8_t mask = 0;
    for (size_t i = 0; i < GROUP_WIDTH; ++i) {
        if (group[i] == EMPTY) {
            mask |= (1u << i);
        }
    }
    return mask;
}

static uint8_t ctz8(uint8_t x) {
    if (x == 0) return 8;
    uint8_t idx = 0;
    while (x) {
        if (x & 1) {
            return idx;
        }
        x >>= 1;
        ++idx;
    }
    return idx;
}

void ls_map_init(ls_map_t *map) {
    if (!map) return;

    map->entries = NULL;
    map->metadatas = NULL;
    map->size = 0;
    map->capacity = 0;
    map->tomb_count = 0;
}

void ls_map_deinit(ls_map_t *map) {
    if (!map) return;

    ls_gc_remove_root(&ls_gc, (void **)&map->entries);
    ls_gc_remove_root(&ls_gc, (void **)&map->metadatas);
    map->entries = NULL;
    map->metadatas = NULL;
    map->size = 0;
    map->capacity = 0;
    map->tomb_count = 0;
}

size_t find_or_prepare(ls_map_t *map, uint64_t hash, ls_any_t key, bool *found) {
    uint8_t h2 = h2_from_hash(hash);
    size_t start_idx = hash & (map->capacity - 1);
    size_t group_start = start_idx & ~(GROUP_WIDTH - 1);

    for (;;) {
        const uint8_t *group = map->metadatas + group_start;

        uint8_t h2_match = match_h2(group, h2);
        while (h2_match) {
            uint8_t offset = ctz8(h2_match);
            size_t idx = group_start + (size_t)offset;
            ls_any_t entry_key = map->entries[idx].key;

            if (is_key_equal(entry_key, key)) {
                *found = true;
                return idx;
            }
            h2_match &= (h2_match - 1);
        }

        uint8_t empty_mask = match_empty(group);
        if (empty_mask) {
            uint8_t offset = ctz8(empty_mask);
            size_t idx = group_start + (size_t)offset;
            *found = false;
            return idx;
        }

        group_start = (group_start + GROUP_WIDTH) & (map->capacity - 1);
    }
}

static void rehash(ls_map_t *map, size_t new_capa) {
    ls_map_entry_t *old_entries = map->entries;
    uint8_t *old_metadatas = map->metadatas;
    size_t old_capa = map->capacity;

    ls_gc_add_root(&ls_gc, (void **)&old_entries);
    ls_gc_add_root(&ls_gc, (void **)&old_metadatas);

    ls_map_entry_t *new_entries = (ls_map_entry_t *)ls_gc_alloc(&ls_gc, new_capa * sizeof(ls_map_entry_t), &ls_entry_name);
    ls_gc_add_root(&ls_gc, (void **)&new_entries);
    uint8_t *new_metadatas = (uint8_t *)ls_gc_alloc(&ls_gc, new_capa, NULL);
    ls_gc_add_root(&ls_gc, (void **)&new_metadatas);

    if (!new_entries || !new_metadatas) {
        ls_gc_remove_root(&ls_gc, (void **)&old_entries);
        ls_gc_remove_root(&ls_gc, (void **)&old_metadatas);
        ls_gc_remove_root(&ls_gc, (void **)&new_entries);
        ls_gc_remove_root(&ls_gc, (void **)&new_metadatas);
        return;
    }

    memset(new_metadatas, EMPTY, new_capa);

    map->entries = new_entries;
    map->metadatas = new_metadatas;
    map->capacity = new_capa;
    map->tomb_count = 0;
    map->size = 0;

    for (size_t i = 0; i < old_capa; ++i) {
        uint8_t meta = old_metadatas[i];
        if (meta == EMPTY || meta == TOMB) continue;

        uint64_t hash = hash_any(old_entries[i].key);

        bool found;
        size_t idx = find_or_prepare(map, hash, old_entries[i].key, &found);
        (void)found;

        map->entries[idx] = old_entries[i];
        map->metadatas[idx] = h2_from_hash(hash);
        ++map->size;
    }

    ls_gc_add_root(&ls_gc, (void **)&map->entries);
    ls_gc_add_root(&ls_gc, (void **)&map->metadatas);

    ls_gc_remove_root(&ls_gc, (void **)&old_entries);
    ls_gc_remove_root(&ls_gc, (void **)&old_metadatas);
    ls_gc_remove_root(&ls_gc, (void **)&new_entries);
    ls_gc_remove_root(&ls_gc, (void **)&new_metadatas);
}

void ls_map_insert(ls_map_t *map, ls_any_t key, ls_any_t value) {
    if (!map) return;

    if (map->size + map->tomb_count + 1 > (size_t)(map->capacity * LOAD_FACTOR)) {
        size_t new_capa = map->capacity == 0 ? 4 : map->capacity << 1;
        rehash(map, new_capa);
        if (map->capacity == 0){
            return;
        }
    }

    uint64_t hash = hash_any(key);
    bool found;
    size_t idx = find_or_prepare(map, hash, key, &found);

    map->entries[idx].key = key;
    map->entries[idx].value = value;
    map->metadatas[idx] = h2_from_hash(hash);

    if (!found) {
        ++map->size;
    }
}

bool ls_map_erase(ls_map_t *map, ls_any_t key) {
    if (!map || map->capacity == 0) return false;

    uint64_t hash = hash_any(key);
    uint8_t h2 = h2_from_hash(hash);
    size_t group_start = (hash & (map->capacity - 1)) & ~(GROUP_WIDTH - 1);

    for (;;) {
        const uint8_t *group = map->metadatas + group_start;
        uint8_t h2_match = match_h2(group, h2);

        while (h2_match) {
            uint8_t offset = ctz8(h2_match);
            size_t idx = group_start + (size_t)offset;
            if (is_key_equal(map->entries[idx].key, key)) {
                map->metadatas[idx] = TOMB;
                --map->size;
                ++map->tomb_count;
                return true;
            }
            h2_match &= (h2_match - 1);
        }

        if (match_empty(group))  {
            return false;
        }
        group_start = (group_start + GROUP_WIDTH) & (map->capacity - 1);
    }
}

bool ls_map_contains(ls_map_t *map, ls_any_t key) {
    if (!map || map->capacity == 0) return false;

    uint64_t hash = hash_any(key);
    uint8_t h2 = h2_from_hash(hash);
    size_t group_start = (hash & (map->capacity - 1)) & ~(GROUP_WIDTH - 1);

    for (;;) {
        const uint8_t *group = map->metadatas + group_start;
        uint8_t h2_match = match_h2(group, h2);

        while (h2_match) {
            uint8_t offset = ctz8(h2_match);
            size_t idx = group_start + (size_t)offset;
            if (is_key_equal(map->entries[idx].key, key)) {
                return true;
            }
            h2_match &= (h2_match - 1);
        }

        if (match_empty(group)) {
            return false;
        }
        group_start = (group_start + GROUP_WIDTH) & (map->capacity - 1);
    }
}


ls_any_t ls_map_at(ls_map_t *map, ls_any_t key) {
    if (!map) {
        fprintf(stderr, "Invalid map argument in ls_map_at.");
        /* temp */
        return LS_NULL_ANY; /* In the future, I will change it. */
    }
    if (map->capacity == 0) {
        fprintf(stderr, "Key not found in map");
        /* temp */
        return LS_NULL_ANY; /* In the future, I will change it. */
    }

    uint64_t hash = hash_any(key);
    uint8_t h2 = h2_from_hash(hash);
    size_t group_start = (hash & (map->capacity - 1)) & ~(GROUP_WIDTH - 1);

    for (;;) {
        const uint8_t *group = map->metadatas + group_start;
        uint8_t h2_match = match_h2(group, h2);

        while (h2_match) {
            uint8_t offset = ctz8(h2_match);
            size_t idx = group_start + (size_t)offset;
            if (is_key_equal(map->entries[idx].key, key)) {
                return map->entries[idx].value;
            }
            h2_match &= (h2_match - 1);
        }

        if (match_empty(group)) {
            fprintf(stderr, "Key not found in map");
            /* temp */
            return LS_NULL_ANY; /* In the future, I will change it. */
        }
        group_start = (group_start + GROUP_WIDTH) & (map->capacity - 1);
    }
}