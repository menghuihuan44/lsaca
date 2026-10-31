// hashmap_i64: 1,000,000 integer-key inserts, 1,000,000 present lookups,
// 1,000,000 absent lookups and 100,000 deletions in an open-addressing table;
// prints the folded checksum, the final size and the number of hits.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MOD 1000000007LL
#define KEY_EMPTY INT64_MIN
#define KEY_TOMB INT64_MAX

typedef struct {
    int64_t key;
    int64_t value;
} Slot;

typedef struct {
    Slot *slots;
    size_t cap;
    size_t len;
} Map;

static void *xcalloc(size_t count, size_t size) {
    void *memory = calloc(count, size);
    if (memory == NULL) {
        fprintf(stderr, "hashmap_i64: out of memory\n");
        exit(1);
    }
    return memory;
}

static uint64_t mix64(uint64_t x) {
    x ^= x >> 30;
    x *= UINT64_C(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x *= UINT64_C(0x94d049bb133111eb);
    x ^= x >> 31;
    return x;
}

static int64_t key_at(int64_t i) {
    return (int64_t)(((uint64_t)i * UINT64_C(2654435761)) % UINT64_C(2147483648));
}

static int64_t mix(int64_t fold, int64_t value) {
    return (fold * 131 + value) % MOD;
}

static void map_init(Map *map) {
    map->cap = 16;
    map->len = 0;
    map->slots = (Slot *)xcalloc(map->cap, sizeof(Slot));
    for (size_t i = 0; i < map->cap; i++) {
        map->slots[i].key = KEY_EMPTY;
    }
}

static void map_free(Map *map) {
    free(map->slots);
    map->slots = NULL;
}

static size_t map_slot(const Map *map, int64_t key) {
    size_t mask = map->cap - 1;
    size_t slot = (size_t)mix64((uint64_t)key) & mask;
    size_t tomb = SIZE_MAX;
    for (;;) {
        int64_t found = map->slots[slot].key;
        if (found == KEY_EMPTY) {
            return tomb != SIZE_MAX ? tomb : slot;
        }
        if (found == key) {
            return slot;
        }
        if (found == KEY_TOMB && tomb == SIZE_MAX) {
            tomb = slot;
        }
        slot = (slot + 1) & mask;
    }
}

static void map_grow(Map *map) {
    size_t old_cap = map->cap;
    Slot *old_slots = map->slots;
    map->cap = old_cap * 2;
    map->slots = (Slot *)xcalloc(map->cap, sizeof(Slot));
    for (size_t i = 0; i < map->cap; i++) {
        map->slots[i].key = KEY_EMPTY;
    }
    for (size_t i = 0; i < old_cap; i++) {
        int64_t key = old_slots[i].key;
        size_t mask = map->cap - 1;
        size_t slot;
        if (key == KEY_EMPTY || key == KEY_TOMB) {
            continue;
        }
        slot = (size_t)mix64((uint64_t)key) & mask;
        while (map->slots[slot].key != KEY_EMPTY) {
            slot = (slot + 1) & mask;
        }
        map->slots[slot] = old_slots[i];
    }
    free(old_slots);
}

static void map_set(Map *map, int64_t key, int64_t value) {
    size_t slot;
    if ((map->len + 1) * 2 >= map->cap) {
        map_grow(map);
    }
    slot = map_slot(map, key);
    if (map->slots[slot].key != key) {
        map->len += 1;
    }
    map->slots[slot].key = key;
    map->slots[slot].value = value;
}

static int map_get(const Map *map, int64_t key, int64_t *value) {
    size_t slot = map_slot(map, key);
    if (map->slots[slot].key != key) {
        return 0;
    }
    *value = map->slots[slot].value;
    return 1;
}

static int map_remove(Map *map, int64_t key, int64_t *value) {
    size_t slot = map_slot(map, key);
    if (map->slots[slot].key != key) {
        return 0;
    }
    *value = map->slots[slot].value;
    map->slots[slot].key = KEY_TOMB;
    map->len -= 1;
    return 1;
}

int main(void) {
    const int64_t n = 1000000;
    Map map;
    int64_t fold = 0;
    int64_t hits = 0;
    int64_t i;
    int64_t d;
    map_init(&map);
    for (i = 0; i < n; i++) {
        int64_t key = key_at(i);
        map_set(&map, key, (key % 10007) + 1);
    }
    for (i = 0; i < n; i++) {
        int64_t value = 0;
        if (map_get(&map, key_at(i), &value)) {
            hits += 1;
            fold = mix(fold, value);
        }
    }
    for (i = 0; i < n; i++) {
        int64_t absent = 2147483648LL + i;
        int64_t value = 0;
        if (map_get(&map, absent, &value)) {
            fold = mix(fold, value);
        } else {
            fold = mix(fold, absent % 97);
        }
    }
    for (d = 0; d < 100000; d++) {
        int64_t value = 0;
        if (map_remove(&map, key_at(d * 10), &value)) {
            fold = mix(fold, value);
        } else {
            fold = mix(fold, 1);
        }
    }
    printf("checksum=%lld,%lld,%lld\n", (long long)fold, (long long)map.len,
           (long long)hits);
    map_free(&map);
    return 0;
}
