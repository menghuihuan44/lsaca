#include "src/base/lsaca_array.h"
#include "src/base/lsaca_gc.h"

void ls_array_init(ls_array_t *array) {
    if (!array) return;

    array->values = NULL;
    array->count = 0;
    array->capacity = 0;
}

void ls_array_deinit(ls_array_t *array) {
    if (!array) return;

    ls_gc_remove_root(&ls_gc, (void **)&array->values);
    array->count = 0;
    array->capacity = 0;
}

void ls_array_append(ls_array_t *array, ls_any_t value) {
    if (!array) return;

    if (array->capacity < array->count + 1) {
        ls_any_t *old_values = array->values;
        size_t old_capa = array->capacity == 0 ? 1 : array->capacity;

        array->values = (ls_any_t *)ls_gc_realloc(&ls_gc, array->values, old_capa << 1);
        if (!array->values) {
            array->values = old_values;
            return;
        }

        if (array->capacity == 0) {
            ls_gc_add_root(&ls_gc, (void **)&array->values);
        }
        array->capacity = old_capa << 1;
    }

    array->values[array->count] = value;
    ++array->count;
}