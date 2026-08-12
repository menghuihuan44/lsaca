#ifndef LSACA_GC_H
#define LSACA_GC_H

#include "src/lsaca_common.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    size_t obj_size;
    size_t ref_count;
    size_t *ref_offsets;
} ls_gc_type_t;

typedef struct {
    size_t size;
    ls_gc_type_t *type;
    void *forward_addr;
    bool forwarded;
} ls_gc_header_t;

typedef struct ls_gc_root_t {
    void **ptr;
    struct ls_gc_root_t *next;
} ls_gc_root_t;

typedef struct {
    void *from_space;
    void *to_space;
    size_t space_size;
    size_t next_gc;

    void *alloc_ptr;
    void *alloc_end;
    void *scan_ptr;

    ls_gc_root_t *roots;
} ls_gc_t;

extern ls_gc_t ls_gc;

void ls_gc_init(ls_gc_t *gc);
void ls_gc_deinit(ls_gc_t *gc);

void *ls_gc_alloc(ls_gc_t *gc, size_t size, ls_gc_type_t *type);
void *ls_gc_realloc(ls_gc_t *gc, void *ptr, size_t new_size);

void ls_gc_add_root(ls_gc_t *gc, void **ptr);
void ls_gc_remove_root(ls_gc_t *gc, void **ptr);
void ls_gc_collect(ls_gc_t *gc);

#define LS_GC_TYPE(name, struct_type, ...) \
    static size_t name##_offsets[] = { __VA_ARGS__ }; \
    static ls_gc_type_t name = { \
        .obj_size = sizeof(struct_type), \
        .ref_count = sizeof(name##_offsets) / sizeof(size_t), \
        .ref_offsets = name##_offsets \
    };

/*
 * Example usage:
 * typedef struct node_t {
 *    int value;
 *    struct node_t *left;
 *    struct node_t *right;
 * } node_t;  
 *
 * LS_GC_TYPE(node_type, node_t, offsetof(node_t, left), offsetof(node_t, right));
 */

#ifdef __cplusplus
}
#endif

#endif /* LSACA_GC_H */