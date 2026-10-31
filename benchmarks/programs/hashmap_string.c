// hashmap_string: 200,000 distinct generated String keys inserted into an
// open-addressing table with strdup'd keys, then 200,000 present lookups,
// 200,000 absent lookups and 20,000 deletions; prints the folded checksum,
// final size and successful lookups.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MOD 1000000007LL
#define KEY_EMPTY ((char *)0)
#define KEY_TOMB ((char *)1)

typedef struct {
    char *key;
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
        fprintf(stderr, "hashmap_string: out of memory\n");
        exit(1);
    }
    return memory;
}

static uint64_t fnv1a(const char *text) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325);
    while (*text != '\0') {
        hash ^= (unsigned char)*text;
        hash *= UINT64_C(0x100000001b3);
        text += 1;
    }
    return hash;
}

static char *dup_key(const char *text) {
    size_t count = strlen(text) + 1;
    char *copy = (char *)malloc(count);
    if (copy == NULL) {
        fprintf(stderr, "hashmap_string: out of memory\n");
        exit(1);
    }
    memcpy(copy, text, count);
    return copy;
}

static void key_text(char buffer[32], int64_t i) {
    (void)snprintf(buffer, 32, "k%lld-%lld", (long long)(i % 1000), (long long)i);
}

static int64_t mix(int64_t fold, int64_t value) {
    return (fold * 131 + value) % MOD;
}

static void map_init(Map *map) {
    map->cap = 16;
    map->len = 0;
    map->slots = (Slot *)xcalloc(map->cap, sizeof(Slot));
}

static void map_free(Map *map) {
    for (size_t i = 0; i < map->cap; i++) {
        char *key = map->slots[i].key;
        if (key != KEY_EMPTY && key != KEY_TOMB) {
            free(key);
        }
    }
    free(map->slots);
    map->slots = NULL;
}

static size_t map_slot(const Map *map, const char *key) {
    size_t mask = map->cap - 1;
    size_t slot = (size_t)fnv1a(key) & mask;
    size_t tomb = SIZE_MAX;
    for (;;) {
        char *found = map->slots[slot].key;
        if (found == KEY_EMPTY) {
            return tomb != SIZE_MAX ? tomb : slot;
        }
        if (found != KEY_TOMB && strcmp(found, key) == 0) {
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
    for (size_t i = 0; i < old_cap; i++) {
        char *key = old_slots[i].key;
        size_t mask = map->cap - 1;
        size_t slot;
        if (key == KEY_EMPTY || key == KEY_TOMB) {
            continue;
        }
        slot = (size_t)fnv1a(key) & mask;
        while (map->slots[slot].key != KEY_EMPTY) {
            slot = (slot + 1) & mask;
        }
        map->slots[slot] = old_slots[i];
    }
    free(old_slots);
}

static void map_set(Map *map, const char *key, int64_t value) {
    size_t slot;
    char *found;
    if ((map->len + 1) * 2 >= map->cap) {
        map_grow(map);
    }
    slot = map_slot(map, key);
    found = map->slots[slot].key;
    if (found != KEY_EMPTY && found != KEY_TOMB) {
        map->slots[slot].value = value;
        return;
    }
    map->slots[slot].key = dup_key(key);
    map->slots[slot].value = value;
    map->len += 1;
}

static int map_get(const Map *map, const char *key, int64_t *value) {
    size_t slot = map_slot(map, key);
    char *found = map->slots[slot].key;
    if (found == KEY_EMPTY || found == KEY_TOMB) {
        return 0;
    }
    *value = map->slots[slot].value;
    return 1;
}

static int map_remove(Map *map, const char *key, int64_t *value) {
    size_t slot = map_slot(map, key);
    char *found = map->slots[slot].key;
    if (found == KEY_EMPTY || found == KEY_TOMB) {
        return 0;
    }
    *value = map->slots[slot].value;
    free(found);
    map->slots[slot].key = KEY_TOMB;
    map->len -= 1;
    return 1;
}

int main(void) {
    const int64_t tokens = 200000;
    Map map;
    char buffer[32];
    int64_t fold = 0;
    int64_t hits = 0;
    int64_t i;
    int64_t d;
    map_init(&map);
    for (i = 0; i < tokens; i++) {
        key_text(buffer, i);
        map_set(&map, buffer, (i % 10007) + 1);
    }
    for (i = 0; i < tokens; i++) {
        int64_t value = 0;
        key_text(buffer, i);
        if (map_get(&map, buffer, &value)) {
            hits += 1;
            fold = mix(fold, value);
        }
    }
    for (i = 0; i < tokens; i++) {
        int64_t value = 0;
        key_text(buffer, tokens + i);
        if (map_get(&map, buffer, &value)) {
            fold = mix(fold, value);
        } else {
            const char *byte = buffer;
            while (*byte != '\0') {
                fold = mix(fold, (int64_t)(unsigned char)*byte);
                byte += 1;
            }
        }
    }
    for (d = 0; d < 20000; d++) {
        int64_t value = 0;
        key_text(buffer, d * 10);
        if (map_remove(&map, buffer, &value)) {
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
