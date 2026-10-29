// hashmap_wordcount: count 200,000 deterministic words (4,096 distinct) in a
// hand-written open-addressing hash map keyed by the word text, then print a
// checksum over the frequencies, the number of distinct keys and the highest
// frequency. The checksum (sum of squared counts) is independent of hash-map
// iteration order.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char *key; // NULL marks an empty slot
    int64_t value;
} Entry;

typedef struct {
    Entry *entries;
    size_t capacity; // power of two
    size_t len;
    int64_t max_value;
} WordMap;

static uint64_t hash_text(const char *text) {
    uint64_t hash = 1469598103934665603ULL;
    for (const unsigned char *p = (const unsigned char *)text; *p != 0; p++) {
        hash ^= (uint64_t)*p;
        hash *= 1099511628211ULL;
    }
    return hash;
}

static void map_init(WordMap *map) {
    map->capacity = 16;
    map->len = 0;
    map->max_value = 0;
    map->entries = (Entry *)calloc(map->capacity, sizeof(Entry));
    if (map->entries == NULL) {
        fprintf(stderr, "out of memory\n");
        exit(1);
    }
}

static void map_free(WordMap *map) {
    for (size_t i = 0; i < map->capacity; i++) {
        free(map->entries[i].key);
    }
    free(map->entries);
}

static void map_grow(WordMap *map) {
    size_t old_capacity = map->capacity;
    Entry *old_entries = map->entries;
    map->capacity = old_capacity * 2;
    map->entries = (Entry *)calloc(map->capacity, sizeof(Entry));
    if (map->entries == NULL) {
        fprintf(stderr, "out of memory\n");
        exit(1);
    }
    size_t mask = map->capacity - 1;
    for (size_t i = 0; i < old_capacity; i++) {
        if (old_entries[i].key == NULL) {
            continue;
        }
        size_t index = (size_t)hash_text(old_entries[i].key) & mask;
        while (map->entries[index].key != NULL) {
            index = (index + 1) & mask;
        }
        map->entries[index] = old_entries[i];
    }
    free(old_entries);
}

static char *copy_text(const char *text) {
    size_t size = strlen(text) + 1;
    char *copy = (char *)malloc(size);
    if (copy == NULL) {
        fprintf(stderr, "out of memory\n");
        exit(1);
    }
    memcpy(copy, text, size);
    return copy;
}

static void map_increment(WordMap *map, const char *key) {
    if ((map->len + 1) * 4 >= map->capacity * 3) {
        map_grow(map);
    }
    size_t mask = map->capacity - 1;
    size_t index = (size_t)hash_text(key) & mask;
    while (map->entries[index].key != NULL) {
        if (strcmp(map->entries[index].key, key) == 0) {
            map->entries[index].value++;
            if (map->entries[index].value > map->max_value) {
                map->max_value = map->entries[index].value;
            }
            return;
        }
        index = (index + 1) & mask;
    }
    map->entries[index].key = copy_text(key);
    map->entries[index].value = 1;
    map->len++;
    if (map->max_value < 1) {
        map->max_value = 1;
    }
}

int main(void) {
    const int64_t tokens = 200000;
    const int64_t distinct = 4096;
    WordMap counts;
    map_init(&counts);
    char word[32];
    for (int64_t i = 0; i < tokens; i++) {
        snprintf(word, sizeof word, "w%lld", (long long)(i % distinct));
        map_increment(&counts, word);
    }
    int64_t checksum = 0;
    for (size_t i = 0; i < counts.capacity; i++) {
        if (counts.entries[i].key == NULL) {
            continue;
        }
        checksum += counts.entries[i].value * counts.entries[i].value;
    }
    printf("checksum=%lld,%lld,%lld\n", (long long)checksum, (long long)counts.len,
           (long long)counts.max_value);
    map_free(&counts);
    return 0;
}
