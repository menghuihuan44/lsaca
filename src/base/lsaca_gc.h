#ifndef LSACA_GC_H
#define LSACA_GC_H

#include "src/lsaca_common.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*ls_gc_ref_visitor)(void **ref_ptr);
typedef void (*ls_gc_trace_func)(void *obj_ptr, ls_gc_ref_visitor visitor);

typedef struct {
    size_t obj_size;
    size_t ref_count;
    const size_t *ref_offsets;
    ls_gc_trace_func trace;
} ls_gc_type_t;

typedef struct ls_gc_header_t {
    struct ls_gc_header_t *next;
    size_t size;
    size_t count;
    const ls_gc_type_t *type;
    uint32_t magic;
    bool marked;
} ls_gc_header_t;

typedef struct ls_gc_root_t {
    void **ptr;
    struct ls_gc_root_t *next;
} ls_gc_root_t;

typedef struct {
    ls_gc_header_t *all_objects;
    ls_gc_root_t *roots;

    size_t total_alloc_size;
    size_t gc_threshold;
} ls_gc_t;

extern ls_gc_t ls_gc;

void ls_gc_init(ls_gc_t *gc);
void ls_gc_deinit(ls_gc_t *gc);

void *ls_gc_alloc(ls_gc_t *gc, size_t size, const ls_gc_type_t *type);
void *ls_gc_realloc(ls_gc_t *gc, void *ptr, size_t new_size);

void ls_gc_add_root(ls_gc_t *gc, void **ptr);
void ls_gc_remove_root(ls_gc_t *gc, void **ptr);
void ls_gc_collect(ls_gc_t *gc);

#define LS_GC_TYPE(name, struct_type, trace_fn, ...) \
    static const size_t name##_offsets[] = { 0, ##__VA_ARGS__ }; \
    static const ls_gc_type_t name = { \
        .obj_size = sizeof(struct_type), \
        .ref_count = (sizeof(name##_offsets) / sizeof(name##_offsets[0])) - 1, \
        .ref_offsets = &name##_offsets[1], \
        .trace = trace_fn \
    };

/*
 * Example usage:
 * typedef struct node_t {
 *    int value;
 *    struct node_t *left;
 *    struct node_t *right;
 * } node_t;  
 *
 * LS_GC_TYPE(node_type, node_t, NULL, offsetof(node_t, left), offsetof(node_t, right));
 */

#ifdef __cplusplus
}
#endif

#endif /* LSACA_GC_H */