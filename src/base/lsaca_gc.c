#include <stdlib.h>
#include <string.h>
#include "src/base/lsaca_gc.h"

#if LS_DEBUG_GC != 0
#include <stdio.h>
#endif

ls_gc_t ls_gc;

#define HEADER_FROM_PTR(ptr) ((ls_gc_header_t *)(ptr) - 1)
#define PTR_FROM_HEADER(h) ((void *)((ls_gc_header_t *)(h) + 1))

#define ALIGN(x) (((x) + alignof(max_align_t) - 1) & ~(alignof(max_align_t) - 1))

#define MIN_GC_THRESHOLD (1024)
#define LS_GC_MAGIC 0xCAFEBABE

static void gc_mark(void *obj_ptr);

static void gc_mark_visit(void **ref_ptr) {
    if (*ref_ptr) {
        gc_mark(*ref_ptr);
    }
}

static void gc_mark(void *obj_ptr) {
    if (!obj_ptr) return;

    ls_gc_header_t *h = HEADER_FROM_PTR(obj_ptr);

    if (h->magic != LS_GC_MAGIC) {
#if LS_DEBUG_GC != 0
        fprintf(stderr, "\n=== GC FATAL ERROR ===\n");
        fprintf(stderr, "gc_mark received an invalid pointer: %p\n", obj_ptr);
        fprintf(stderr, "Header magic mismatch! Expected 0x%X, Got 0x%X\n", LS_GC_MAGIC, h->magic);
        fprintf(stderr, "This is usually caused by:\n");
        fprintf(stderr, "1. Passing an uninitialized pointer to GC.\n");
        fprintf(stderr, "2. Passing an internal pointer (e.g., &obj->field) to GC.\n");
#endif
        abort();
    }

    if (h->marked) return;
    h->marked = true;

    if (h->type) {
        void *cur = PTR_FROM_HEADER(h);
        for (size_t c = 0; c < h->count; ++c) {
            if (h->type->trace) {
                h->type->trace(cur, gc_mark_visit);
            } else {
                for (size_t i = 0; i < h->type->ref_count; ++i) {
                    size_t offset = h->type->ref_offsets[i];
                    if (offset + sizeof(void *) <= h->size) {
                        void **ref_ptr = (void **)((char *)cur + offset);
                        gc_mark_visit(ref_ptr);
                    }
                }
            }
            cur = (void *)((char *)cur + h->type->obj_size);
        }
    }
}

void ls_gc_init(ls_gc_t *gc) {
    if (!gc) return;

    gc->all_objects = NULL;
    gc->total_alloc_size = 0;
    gc->gc_threshold = MIN_GC_THRESHOLD;
    gc->roots = NULL;
}

void ls_gc_deinit(ls_gc_t *gc) {
    if (!gc) return;

    ls_gc_root_t *root = gc->roots;
    while (root) {
        ls_gc_root_t *next = root->next;
        free(root);
        root = next;
    }

    ls_gc_header_t *obj = gc->all_objects;
    while (obj) {
        ls_gc_header_t *next = obj->next;
        free(obj);
        obj = next;
    }

    gc->all_objects = NULL;
    gc->roots = NULL;
    gc->total_alloc_size = 0;
    gc->gc_threshold = MIN_GC_THRESHOLD;
}

void *ls_gc_alloc(ls_gc_t *gc, size_t size, const ls_gc_type_t *type) {
    if (!gc || size == 0) return NULL;
    
    if (gc->total_alloc_size >= gc->gc_threshold) {
        ls_gc_collect(gc);
        if (gc->total_alloc_size >= gc->gc_threshold) {
            gc->gc_threshold = gc->total_alloc_size << 1;
        }
    }

    size_t totol_size = sizeof(ls_gc_header_t) + size;
    totol_size = ALIGN(totol_size);

    ls_gc_header_t *h = (ls_gc_header_t *)malloc(totol_size);
    if (!h) return NULL;

    h->magic = LS_GC_MAGIC;
    h->marked = false;
    h->size = size;
    h->type = type;

    if (type && type->obj_size > 0) {
        h->count = size / type->obj_size;
        if (h->count == 0) h->count = 1;
    } else {
        h->count = 1;
    }

    h->next = gc->all_objects;
    gc->all_objects = h;
    gc->total_alloc_size += totol_size;
    memset(PTR_FROM_HEADER(h), 0, size);

    return PTR_FROM_HEADER(h);
}


void *ls_gc_realloc(ls_gc_t *gc, void *ptr, size_t new_size) {
    if (!gc || new_size == 0) return NULL;
    if (!ptr) return ls_gc_alloc(gc, new_size, NULL);

    ls_gc_header_t *old_h = HEADER_FROM_PTR(ptr);

    if (old_h->magic != LS_GC_MAGIC) {
#if LS_DEBUG_GC != 0
        fprintf(stderr, "GC FATAL ERROR: realloc invalid pointer %p\n", ptr);
#endif
        abort();
    }

    size_t copy_size = old_h->size < new_size ? old_h->size : new_size;
    size_t alloc_size = new_size;
    if (old_h->type && old_h->type->obj_size > alloc_size) {
        alloc_size = old_h->type->obj_size;
    }

    size_t old_total = ALIGN(sizeof(ls_gc_header_t) + old_h->size);
    size_t new_total = ALIGN(sizeof(ls_gc_header_t) + alloc_size);

    ls_gc_header_t *new_h = (ls_gc_header_t *)realloc(old_h, new_total);
    if (!new_h) return NULL;

    new_h->size = alloc_size;
    if (new_h->type && new_h->type->obj_size > 0) {
        new_h->count = alloc_size / new_h->type->obj_size;
        if (new_h->count == 0) new_h->count = 1;
    } else {
        new_h->count = 1;
    }

    new_h->size = alloc_size;

    if (new_h != old_h) {
        if (gc->all_objects == old_h) {
            gc->all_objects = new_h;
        } else {
            ls_gc_header_t *curr = gc->all_objects;
            while (curr && curr->next != old_h) {
                curr = curr->next;
            }
            if (curr) {
                curr->next = new_h;
            } else {
#if LS_DEBUG_GC != 0
                fprintf(stderr, "GC FATAL ERROR: realloc moved an untracked object %p\n", (void *)old_h);
                abort();
#else
                new_h->next = gc->all_objects;
                gc->all_objects = new_h;
#endif
            }
        }
    }

    gc->total_alloc_size -= old_total;
    gc->total_alloc_size += new_total;

    return PTR_FROM_HEADER(new_h);
}

void ls_gc_add_root(ls_gc_t *gc, void **ptr) {
    if (!gc || !ptr) return;

    for (ls_gc_root_t *root = gc->roots; root != NULL; root = root->next) {
        if (root->ptr == ptr) {
            return;
        }
    }

    ls_gc_root_t *new_root = (ls_gc_root_t *)malloc(sizeof(ls_gc_root_t));
    if (!new_root) return;

    new_root->ptr = ptr;
    new_root->next = gc->roots;
    gc->roots = new_root;
}

void ls_gc_remove_root(ls_gc_t *gc, void **ptr) {
    if (!gc || !ptr) return;

    ls_gc_root_t **prev = &gc->roots;
    while (*prev) {
        ls_gc_root_t *curr = *prev;
        if (curr->ptr == ptr) {
            *prev = curr->next;
            free(curr);
            return;
        }
        prev = &curr->next;
    }
}

void ls_gc_collect(ls_gc_t *gc) {
    if (!gc) return;
#if LS_DEBUG_GC != 0
    fprintf(stderr, "GC begin. Total alloc size: %zu\n", gc->total_alloc_size);
#endif

    for (ls_gc_root_t *root = gc->roots; root != NULL; root = root->next) {
        if (*root->ptr) {
            gc_mark(*root->ptr);
        }
    }

    ls_gc_header_t **prev = &gc->all_objects;
    ls_gc_header_t *curr = gc->all_objects;

    while (curr) {
        ls_gc_header_t *next = curr->next;

        if (curr->marked) {
            curr->marked = false;
            prev = &curr->next;
        } else {
            *prev = next;

            size_t total = ALIGN(sizeof(ls_gc_header_t) + curr->size);
            gc->total_alloc_size -= total;

            free(curr);
        }
        curr = next;
    }

#if LS_DEBUG_GC != 0
    fprintf(stderr, "GC end. Total alloc size: %zu\n", gc->total_alloc_size);
#endif
}