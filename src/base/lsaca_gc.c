#include <stdlib.h>
#include <string.h>
#include <stdalign.h>
#include "src/base/lsaca_gc.h"

ls_gc_t ls_gc;

#define HEADER_FROM_PTR(ptr) ((ls_gc_header_t *)(ptr) - 1)
#define PTR_FROM_HEADER(h) ((void *)((ls_gc_header_t *)(h) + 1))

#define ALIGN(x) (((x) + alignof(max_align_t) - 1) & ~(alignof(max_align_t) - 1))

#define MIN_SPACE_SIZE (1024) 

static void *copy_object(ls_gc_t *gc, void *old_ptr) {
    ls_gc_header_t *old_h = HEADER_FROM_PTR(old_ptr);
    if (old_h->forwarded) {
        return old_h->forward_addr;
    }

    size_t total_size = sizeof(ls_gc_header_t) + old_h->size;
    total_size = ALIGN(total_size);

    void *new_ptr = gc->alloc_ptr;
    memcpy(new_ptr, old_h, total_size);

    gc->alloc_ptr = (char *)gc->alloc_ptr + total_size;

    old_h->forwarded = true;
    old_h->forward_addr = PTR_FROM_HEADER(new_ptr);
    
    return PTR_FROM_HEADER(new_ptr);
}

static bool resize_spaces(ls_gc_t *gc, size_t new_space_size) {
    new_space_size = ALIGN(new_space_size);

    void *old_from = gc->from_space;
    void *old_to = gc->to_space;

    void *new_from = malloc(new_space_size);
    void *new_to = malloc(new_space_size);
    if (!new_from || !new_to) {
        free(new_from);
        free(new_to);
        return false;
    }

    size_t live_bytes = (char *)gc->alloc_ptr - (char *)old_from;
    memcpy(new_from, old_from, live_bytes);

    intptr_t delta = (char *)new_from - (char *)old_from;
    for (ls_gc_root_t *root = gc->roots; root != NULL; root = root->next) {
        if (*root->ptr) {
            *root->ptr = (char *)(*root->ptr) + delta;
        }
    }

    void *p = new_from;
    while ((char *)p < (char *)new_from + live_bytes) {
        ls_gc_header_t *h = (ls_gc_header_t *)p;
        if (h->type) {
            void *obj_ptr = PTR_FROM_HEADER(h);
            for (size_t i = 0; i < h->type->ref_count; ++i) {
                size_t offset = h->type->ref_offsets[i];
                void **ref_ptr = (void **)((char *)obj_ptr + offset);
                if (*ref_ptr) {
                    *ref_ptr = (char *)(*ref_ptr) + delta;
                }
            }
        }
        size_t total_size = sizeof(ls_gc_header_t) + h->size;
        total_size = ALIGN(total_size);
        p = (char *)p + total_size;
    }

    gc->from_space = new_from;
    gc->to_space = new_to;
    gc->space_size = new_space_size;
    gc->alloc_ptr = (char *)new_from + live_bytes;
    gc->alloc_end = (char *)new_from + new_space_size;
    gc->scan_ptr = NULL;

    free(old_from);
    free(old_to);

    return true;
}

void ls_gc_init(ls_gc_t *gc) {
    gc->space_size = MIN_SPACE_SIZE;
    gc->next_gc = MIN_SPACE_SIZE << 1;
    gc->from_space = malloc(gc->space_size);
    gc->to_space = malloc(gc->space_size);
    if (!gc->from_space || !gc->to_space) {
        free(gc->from_space);
        free(gc->to_space);
    }

    gc->alloc_ptr = gc->from_space;
    gc->alloc_end = (char *)gc->from_space + gc->space_size;
    gc->scan_ptr = NULL;
    gc->roots = NULL;
}

void ls_gc_deinit(ls_gc_t *gc) {
    ls_gc_root_t *root = gc->roots;
    while (root) {
        ls_gc_root_t *next = root->next;
        free(root);
        root = next;
    }

    free(gc->from_space);
    free(gc->to_space);
}

void *ls_gc_alloc(ls_gc_t *gc, size_t size, ls_gc_type_t *type) {
    if (!gc || size == 0) return NULL;
    
    size_t total_size = sizeof(ls_gc_header_t) + size;
    total_size = ALIGN(total_size);

    while ((char *)gc->alloc_ptr + total_size > (char *)gc->alloc_end) {
        size_t new_space_size = gc->space_size << 1;
        if (!resize_spaces(gc, new_space_size)) {
            return NULL;
        }
    }

    if (gc->space_size > gc->next_gc) {
        ls_gc_collect(gc);
    }

    ls_gc_header_t *h = (ls_gc_header_t *)gc->alloc_ptr;
    h->size = size;
    h->type = type;
    h->forwarded = false;
    h->forward_addr = NULL;

    memset((PTR_FROM_HEADER(h)), 0, size);

    gc->alloc_ptr = (char *)gc->alloc_ptr + total_size;

    return PTR_FROM_HEADER(h);
}


void *ls_gc_realloc(ls_gc_t *gc, void *ptr, size_t new_size) {
    if (!gc || new_size == 0) return NULL;
    if (!ptr) return ls_gc_alloc(gc, new_size, NULL);

    ls_gc_header_t *old_h = HEADER_FROM_PTR(ptr);
    size_t copy_size = old_h->size < new_size ? old_h->size : new_size;

    void *new_ptr;
    if (old_h->type) {
        new_ptr = ls_gc_alloc(gc, new_size, old_h->type);
    } else {
        new_ptr = ls_gc_alloc(gc, new_size, NULL);
    }
    if (!new_ptr) return NULL;

    memcpy(new_ptr, ptr, copy_size);

    return new_ptr;
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

    void *tmp = gc->from_space;
    gc->from_space = gc->to_space;
    gc->to_space = tmp;

    gc->alloc_ptr = gc->from_space;
    gc->alloc_end = (char *)gc->from_space + gc->space_size;
    gc->scan_ptr = gc->from_space;

    for (ls_gc_root_t *root = gc->roots; root != NULL; root = root->next) {
        if (*root->ptr) {
            *root->ptr = copy_object(gc, *root->ptr);
        }
    }

    while ((char *)gc->scan_ptr < (char *)gc->alloc_ptr) {
        ls_gc_header_t *h = (ls_gc_header_t *)gc->scan_ptr;
        if (h->type) {
            void *obj_ptr = PTR_FROM_HEADER(h);
            for (size_t i = 0; i < h->type->ref_count; ++i) {
                size_t offset = h->type->ref_offsets[i];
                void **ref_ptr = (void **)((char *)obj_ptr + offset);
                if (*ref_ptr) {
                    *ref_ptr = copy_object(gc, *ref_ptr);
                }
            }
        }
        size_t total_size = sizeof(ls_gc_header_t) + h->size;
        total_size = ALIGN(total_size);
        gc->scan_ptr = (char *)gc->scan_ptr + total_size;
    }

    size_t live_bytes = (char *)gc->alloc_ptr - (char *)gc->from_space;
    size_t threshold_high = (gc->space_size * 3) >> 2;
    size_t threshold_low = gc->space_size >> 2;

    if (live_bytes > threshold_high) {
        resize_spaces(gc, gc->space_size << 1);
    } else if (live_bytes < threshold_low && (gc->space_size >> 1) > MIN_SPACE_SIZE) {
        resize_spaces(gc, gc->space_size >> 1);
    }
}