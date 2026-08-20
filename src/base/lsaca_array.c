#include "src/base/lsaca_array.h"
#include "src/base/lsaca_gc.h"

static void ls_any_trace(void *obj_ptr, ls_gc_ref_visitor visitor) {
    ls_any_t *value = (ls_any_t *)obj_ptr;
    switch (LS_GET_TYPE(*value)) {
        case LS_ANY_STR: visitor((void **)&LS_AS_STR(*value)); break;
        default: break;
    }
}

LS_GC_TYPE(ls_any_name, ls_any_t, ls_any_trace);

void ls_array_init(ls_array_t *array) {
    if (!array) return;

    array->values = NULL;
    array->size = 0;
    array->capacity = 0;
}

void ls_array_deinit(ls_array_t *array) {
    if (!array) return;

    ls_gc_remove_root(&ls_gc, (void **)&array->values);
    array->size = 0;
    array->capacity = 0;
}

void ls_array_append(ls_array_t *array, ls_any_t value) {
    if (!array) return;

    if (array->capacity < array->size + 1) {
        ls_any_t *old_values = array->values;
        size_t new_capa = array->capacity == 0 ? 1 : (array->capacity << 1);

        if (array->capacity == 0) {
            array->values = (ls_any_t *)ls_gc_alloc(&ls_gc, new_capa * sizeof(ls_any_t), &ls_any_name);
            ls_gc_add_root(&ls_gc, (void **)&array->values);
        } else {
            array->values = (ls_any_t *)ls_gc_realloc(&ls_gc, array->values, new_capa * sizeof(ls_any_t));
        }
        if (!array->values) {
            array->values = old_values;
            ls_gc_remove_root(&ls_gc, (void **)&array->values);
            return;
        }

        array->capacity = new_capa;
    }

    array->values[array->size] = value;
    ++array->size;
}