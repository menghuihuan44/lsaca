#include "src/base/lsaca_object.h"
#include "src/base/lsaca_gc.h"

static void ls_function_trace(void *obj_ptr, ls_gc_ref_visitor visitor) {
    ls_function_t *function = (ls_function_t *)obj_ptr;
    visitor((void **)&function->name);
    visitor((void **)&function->chunk.codes);
    visitor((void **)&function->chunk.lines);
    visitor((void **)&function->chunk.constants.values);
}

LS_GC_TYPE(ls_function_name, ls_function_t, ls_function_trace);

ls_function_t *ls_function_create(void) {
    ls_function_t *function = (ls_function_t *)ls_gc_alloc(&ls_gc, sizeof(ls_function_t), &ls_function_name);
    if (!function) {
        return NULL;
    }
    ls_chunk_init(&function->chunk);
    function->name = NULL;
    function->arity = 0;
    return function;
}