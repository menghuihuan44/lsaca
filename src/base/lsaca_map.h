#ifndef LSACA_MAP_H
#define LSACA_MAP_H

#include "src/base/lsaca_any.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    ls_any_t key;
    ls_any_t value;
} ls_map_entry_t;

typedef struct {
    ls_map_entry_t *entries;
    uint8_t *metadatas;
    size_t size;
    size_t capacity;
    size_t tomb_count;
} ls_map_t;

void ls_map_init(ls_map_t *map);
void ls_map_deinit(ls_map_t *map);

void ls_map_insert(ls_map_t *map, ls_any_t key, ls_any_t value);
bool ls_map_erase(ls_map_t *map, ls_any_t key);
bool ls_map_contains(ls_map_t *map, ls_any_t key);
ls_any_t ls_map_at(ls_map_t *map, ls_any_t key);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_MAP_H */