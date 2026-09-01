#ifndef LSACA_ARRAY_H
#define LSACA_ARRAY_H

#include "src/base/lsaca_any.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    ls_any_t *values;
    size_t size;
    size_t capacity;
} ls_array_t;

void ls_array_init(ls_array_t *array);
void ls_array_deinit(ls_array_t *array);

void ls_array_append(ls_array_t *array, ls_any_t value);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_ARRAY_H */