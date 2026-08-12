#ifndef LSACA_ARRAY_H
#define LSACA_ARRAY_H

#include "src/base/lsaca_any.h"

#ifdef __cplusplus
extern "C" {
#endif

struct ls_array_t {
    ls_any_t *values;
    size_t count;
    size_t capacity;
};

void ls_array_init(ls_array_t *array);
void ls_array_deinit(ls_array_t *array);

void ls_array_append(ls_array_t *array, ls_any_t value);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_ARRAY_H */