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

#define MIN_SPACE_SIZE (1024) 

typedef struct {
    ls_gc_t *gc;
    void *old_from;
    void *old_from_end;
    intptr_t delta;
} ls_gc_trace_ctx_t;

static ls_gc_trace_ctx_t g_trace_ctx;

static void *copy_object(ls_gc_t *gc, void *old_ptr);

static void gc_collect_visit(void **ref_ptr) {
    if (*ref_ptr) {
        if (*ref_ptr >= g_trace_ctx.old_from && *ref_ptr < g_trace_ctx.old_from_end) {
            *ref_ptr = copy_object(g_trace_ctx.gc, *ref_ptr);
        }
#if LS_DEBUG_GC != 0
        else {
            fprintf(stderr, "\n=== GC Warning ===\n");
            fprintf(stderr, "Found invalid pointer value: %p\n", *ref_ptr);
            fprintf(stderr, "Stored at address: %p\n", ref_ptr);
            
            if (ref_ptr >= (void **)g_trace_ctx.gc->from_space && 
                ref_ptr < (void **)g_trace_ctx.gc->alloc_end) {
                fprintf(stderr, "Location: Inside a GC heap object.\n");
            } else {
                fprintf(stderr, "Location: In a Root (stack or global variable).\n");
            }
        }
#endif
    }
}

static void gc_resize_visit(void **ref_ptr) {
    if (*ref_ptr) {
        if (*ref_ptr >= g_trace_ctx.old_from && *ref_ptr < g_trace_ctx.old_from_end) {
            *ref_ptr = (char *)(*ref_ptr) + g_trace_ctx.delta;
        }
    }
}

static void *copy_object(ls_gc_t *gc, void *old_ptr) {
    if (!g_trace_ctx.old_from || !(old_ptr >= g_trace_ctx.old_from && old_ptr < g_trace_ctx.old_from_end)) {
#if LS_DEBUG_GC != 0
        fprintf(stderr, "\n=== GC FATAL ERROR ===\n");
        fprintf(stderr, "copy_object received an invalid pointer: %p\n", old_ptr);
        fprintf(stderr, "Valid range was: [%p, %p)\n", g_trace_ctx.old_from, g_trace_ctx.old_from_end);
        for (ls_gc_root_t *root = gc->roots; root != NULL; root = root->next) {
            if (*root->ptr == old_ptr) {
                fprintf(stderr, "CULPRIT FOUND: Root at address %p holds this invalid pointer!\n", root->ptr);
                break;
            }
        }
#endif
        abort();
    }

    ls_gc_header_t *old_h = HEADER_FROM_PTR(old_ptr);
    if (old_h->forwarded) {
        return old_h->forward_addr;
    }

    if (old_h->size > gc->space_size) {
#if LS_DEBUG_GC != 0
        fprintf(stderr, "\n=== GC FATAL ERROR ===\n");
        fprintf(stderr, "Object header corrupted at %p! size is %zu (space_size is %zu)\n", 
                old_ptr, old_h->size, gc->space_size);
#endif
        abort();
    }

    size_t total_size = sizeof(ls_gc_header_t) + old_h->size;
    total_size = ALIGN(total_size);

    ls_gc_header_t *new_h = (ls_gc_header_t *)gc->alloc_ptr;
    memcpy(new_h, old_h, total_size);

    new_h->forwarded = false;
    new_h->forward_addr = NULL;

    gc->alloc_ptr = (char *)gc->alloc_ptr + total_size;

    old_h->forwarded = true;
    old_h->forward_addr = PTR_FROM_HEADER(new_h);
    
    return PTR_FROM_HEADER(new_h);
}

static bool resize_spaces(ls_gc_t *gc, size_t new_space_size) {
#if LS_DEBUG_GC != 0
    printf("GC start resize spaces to %zu.\n", new_space_size);
#endif
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

    g_trace_ctx.gc = gc;
    g_trace_ctx.old_from = old_from;
    g_trace_ctx.old_from_end = (char *)old_from + live_bytes;
    g_trace_ctx.delta = delta;

    for (ls_gc_root_t *root = gc->roots; root != NULL; root = root->next) {
        if (*root->ptr) {
            gc_resize_visit(root->ptr);
        }
    }

    void *p = new_from;
    while ((char *)p < (char *)new_from + live_bytes) {
        ls_gc_header_t *h = (ls_gc_header_t *)p;

        if (h->size > new_space_size) {
#if LS_DEBUG_GC != 0
            fprintf(stderr, "\n=== GC FATAL ERROR ===\n");
            fprintf(stderr, "Heap corrupted during resize at %p! size is %zu\n", p, h->size);
#endif
            abort();
        }

        if (h->type) {
            void *obj_ptr = PTR_FROM_HEADER(h);
            for (size_t c = 0; c < h->count; ++c) {
                if (h->type->trace) {
                    h->type->trace(obj_ptr, gc_resize_visit);
                } else {
                    for (size_t i = 0; i < h->type->ref_count; ++i) {
                        size_t offset = h->type->ref_offsets[i];
                        if (offset + sizeof(void *) <= h->size) {
                            void **ref_ptr = (void **)((char *)obj_ptr + offset);
                            gc_resize_visit(ref_ptr);
                        }
                    }
                }
                obj_ptr = (void *)((char *)obj_ptr + h->type->obj_size);
            }
        }
        h->forwarded = false;
        h->forward_addr = NULL;

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

#if LS_DEBUG_GC != 0
    printf("GC end resize spaces.\n");
#endif

    return true;
}

void ls_gc_init(ls_gc_t *gc) {
    gc->space_size = MIN_SPACE_SIZE;
    gc->from_space = malloc(gc->space_size);
    gc->to_space = malloc(gc->space_size);
    if (!gc->from_space || !gc->to_space) {
        free(gc->from_space);
        free(gc->to_space);
        return;
    }

    gc->alloc_ptr = gc->from_space;
    gc->alloc_end = (char *)gc->from_space + gc->space_size;
    gc->scan_ptr = NULL;
    gc->roots = NULL;

    memset(&g_trace_ctx, 0, sizeof(g_trace_ctx));
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

void *ls_gc_alloc(ls_gc_t *gc, size_t size, const ls_gc_type_t *type) {
    if (!gc || size == 0) return NULL;
    
    size_t total_size = sizeof(ls_gc_header_t) + size;
    total_size = ALIGN(total_size);
    
    if ((char *)gc->alloc_ptr + total_size > (char *)gc->alloc_end) {
        ls_gc_collect(gc);
    }

    while ((char *)gc->alloc_ptr + total_size > (char *)gc->alloc_end) {
        size_t new_space_size = gc->space_size << 1;
        if (new_space_size < total_size) {
            new_space_size = total_size << 1;
        }
        if (!resize_spaces(gc, new_space_size)) {
            return NULL;
        }
    }

    ls_gc_header_t *h = (ls_gc_header_t *)gc->alloc_ptr;
    h->size = size;
    h->type = type;
    h->forwarded = false;
    h->forward_addr = NULL;

    if (type && type->obj_size > 0) {
        h->count = size / type->obj_size;
        if (h->count == 0) h->count = 1;
    } else {
        h->count = 1;
    }

    memset((PTR_FROM_HEADER(h)), 0, size);

    gc->alloc_ptr = (char *)gc->alloc_ptr + total_size;

    return PTR_FROM_HEADER(h);
}


void *ls_gc_realloc(ls_gc_t *gc, void *ptr, size_t new_size) {
    if (!gc || new_size == 0) return NULL;
    if (!ptr) return ls_gc_alloc(gc, new_size, NULL);

    ls_gc_add_root(gc, (void **)&ptr);
    ls_gc_header_t *old_h = HEADER_FROM_PTR(ptr);
    size_t copy_size = old_h->size < new_size ? old_h->size : new_size;

    size_t alloc_size = new_size;
    if (old_h->type && old_h->type->obj_size > alloc_size) {
        alloc_size = old_h->type->obj_size;
    }

    void *new_ptr = ls_gc_alloc(gc, alloc_size, old_h->type);
    ls_gc_remove_root(gc, (void **)&ptr);
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
#if LS_DEBUG_GC != 0
    printf("GC begin.\n");
#endif
    
    void *old_from = gc->from_space;
    void *old_from_end = gc->alloc_ptr; 

    g_trace_ctx.gc = gc;
    g_trace_ctx.old_from = old_from;
    g_trace_ctx.old_from_end = old_from_end;
    g_trace_ctx.delta = 0;

    void *tmp = gc->from_space;
    gc->from_space = gc->to_space;
    gc->to_space = tmp;

    gc->alloc_ptr = gc->from_space;
    gc->alloc_end = (char *)gc->from_space + gc->space_size;
    gc->scan_ptr = gc->from_space;

    for (ls_gc_root_t *root = gc->roots; root != NULL; root = root->next) {
        if (*root->ptr) {
            gc_collect_visit(root->ptr);
        }
    }

    while ((char *)gc->scan_ptr < (char *)gc->alloc_ptr) {
        ls_gc_header_t *h = (ls_gc_header_t *)gc->scan_ptr;
        if (h->type) {
            void *obj_ptr = PTR_FROM_HEADER(h);
            for (size_t c = 0; c < h->count; ++c) {
                if (h->type->trace) {
                    h->type->trace(obj_ptr, gc_collect_visit);
                } else {
                    for (size_t i = 0; i < h->type->ref_count; ++i) {
                        size_t offset = h->type->ref_offsets[i];
                        if (offset + sizeof(void *) <= h->size) {
                            void **ref_ptr = (void **)((char *)obj_ptr + offset);
                            gc_collect_visit(ref_ptr);
                        }
                    }
                }
                obj_ptr = (void *)((char *)obj_ptr + h->type->obj_size);
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

#if LS_DEBUG_GC != 0
    printf("GC end.\n");
    printf("GC's space_size: %zu\n", gc->space_size);
#endif
}