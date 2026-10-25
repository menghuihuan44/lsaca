/* lsaca C17 runtime: vector translation unit.
 *
 * Generic byte-level vector primitives used by generated code for Vec<T> and
 * the frozen LsVec_String instantiation. The allocator/collector and the shared
 * private contract live in lsaca_rt.c / lsaca_rt_internal.h.
 */

#include "lsaca_rt_internal.h"

/* ====================================================================== */
/* Generic vector primitives                                              */
/* ====================================================================== */

void* ls_vec_alloc(size_t capacity, size_t elem_size) {
    void* data;

    if (capacity == 0 || elem_size == 0) {
        return NULL;
    }
    if (capacity > SIZE_MAX / elem_size) {
        ls_rt_alloc_failure(capacity);
    }
    data = calloc(capacity, elem_size);
    if (data == NULL) {
        ls_rt_alloc_failure(capacity * elem_size);
    }
    return data;
}

void ls_vec_reserve(void** data, size_t* cap, size_t len, size_t needed,
                    size_t elem_size) {
    size_t newcap;
    size_t bytes;
    void* replacement;

    (void)len;
    if (elem_size == 0) {
        if (needed > *cap) {
            *cap = needed;
        }
        return;
    }
    if (needed <= *cap) {
        return;
    }
    newcap = *cap == 0 ? 4 : *cap;
    while (newcap < needed) {
        if (newcap > SIZE_MAX / 2) {
            newcap = needed;
            break;
        }
        newcap *= 2;
    }
    if (newcap > SIZE_MAX / elem_size) {
        ls_rt_alloc_failure(newcap);
    }
    bytes = newcap * elem_size;
    replacement = realloc(*data, bytes);
    if (replacement == NULL) {
        ls_rt_alloc_failure(bytes);
    }
    *data = replacement;
    *cap = newcap;
}

void ls_vec_grow(void** data, size_t* cap, size_t len, size_t elem_size) {
    if (len == SIZE_MAX) {
        ls_rt_alloc_failure(SIZE_MAX);
    }
    ls_vec_reserve(data, cap, len, len + 1, elem_size);
}

void* ls_vec_clone_bytes(const void* data, size_t len, size_t elem_size) {
    void* copy;

    if (len == 0 || elem_size == 0 || data == NULL) {
        return NULL;
    }
    if (len > SIZE_MAX / elem_size) {
        ls_rt_alloc_failure(len);
    }
    copy = malloc(len * elem_size);
    if (copy == NULL) {
        ls_rt_alloc_failure(len * elem_size);
    }
    memcpy(copy, data, len * elem_size);
    return copy;
}

void ls_vec_insert_bytes(void** data, size_t* len, size_t* cap, size_t index,
                         const void* value, size_t elem_size) {
    char stack_copy[64];
    char* base;
    char* temp;
    size_t bytes;
    size_t tail;
    uintptr_t value_address;
    uintptr_t buffer_address;

    if (elem_size == 0) {
        *len += 1;
        return;
    }
    if (index > *len) {
        index = *len;
    }
    tail = *len - index;
    base = (char*)*data;
    /* Fast path (the Vec::push / Vec::insert hot path): the capacity already
     * has room and "value" does not point into the buffer, so no reallocation
     * can invalidate it and no temporary copy is needed. */
    bytes = *len * elem_size;
    value_address = (uintptr_t)value;
    buffer_address = (uintptr_t)base;
    if (*len < *cap && base != NULL &&
        (value_address < buffer_address ||
         value_address >= buffer_address + bytes)) {
        memmove(base + (index + 1) * elem_size, base + index * elem_size,
                tail * elem_size);
        memcpy(base + index * elem_size, value, elem_size);
        *len += 1;
        return;
    }
    /* Slow path: the buffer has to grow, or "value" aliases the buffer and
     * must be snapshotted before the element shuffle. Values that fit in a
     * small stack buffer avoid the heap allocation entirely. */
    temp = elem_size <= sizeof stack_copy ? stack_copy
                                          : (char*)malloc(elem_size);
    if (temp == NULL) {
        ls_rt_alloc_failure(elem_size);
    }
    memcpy(temp, value, elem_size);
    if (*len == *cap) {
        ls_vec_grow(data, cap, *len, elem_size);
    }
    base = (char*)*data;
    memmove(base + (index + 1) * elem_size, base + index * elem_size,
            tail * elem_size);
    memcpy(base + index * elem_size, temp, elem_size);
    if (temp != stack_copy) {
        free(temp);
    }
    *len += 1;
}

void ls_vec_remove_bytes(void* data, size_t* len, size_t index, void* out,
                         size_t elem_size) {
    char* base = (char*)data;

    if (index >= *len) {
        return;
    }
    if (elem_size == 0) {
        *len -= 1;
        return;
    }
    if (out != NULL) {
        memcpy(out, base + index * elem_size, elem_size);
    }
    memmove(base + index * elem_size, base + (index + 1) * elem_size,
            (*len - index - 1) * elem_size);
    *len -= 1;
}

void ls_vec_extend_bytes(void** data, size_t* len, size_t* cap, const void* src,
                         size_t src_len, size_t elem_size) {
    char* temp;
    size_t bytes;

    if (src_len == 0 || src == NULL) {
        return;
    }
    if (elem_size == 0) {
        *len += src_len;
        return;
    }
    if (src_len > SIZE_MAX - *len) {
        ls_rt_alloc_failure(src_len);
    }
    if (src_len > SIZE_MAX / elem_size) {
        ls_rt_alloc_failure(src_len);
    }
    bytes = src_len * elem_size;
    temp = (char*)malloc(bytes);
    if (temp == NULL) {
        ls_rt_alloc_failure(bytes);
    }
    memcpy(temp, src, bytes);
    ls_vec_reserve(data, cap, *len, *len + src_len, elem_size);
    memcpy((char*)*data + *len * elem_size, temp, bytes);
    *len += src_len;
    free(temp);
}

void ls_vec_reverse_bytes(void* data, size_t len, size_t elem_size) {
    char* base = (char*)data;
    char* temp;
    size_t i;

    if (elem_size == 0 || len < 2 || data == NULL) {
        return;
    }
    temp = (char*)malloc(elem_size);
    if (temp == NULL) {
        ls_rt_alloc_failure(elem_size);
    }
    for (i = 0; i < len / 2; i += 1) {
        memcpy(temp, base + i * elem_size, elem_size);
        memcpy(base + i * elem_size, base + (len - 1 - i) * elem_size,
               elem_size);
        memcpy(base + (len - 1 - i) * elem_size, temp, elem_size);
    }
    free(temp);
}

void ls_vec_free_bytes(void* data) {
    free(data);
}

/* ====================================================================== */
/* LsVec_String operations                                                */
/* ====================================================================== */

static void ls_vec_string_reserve_internal(LsVec_String* v, size_t needed) {
    size_t newcap;
    LsString** replacement;

    if (needed <= v->cap) {
        return;
    }
    newcap = v->cap == 0 ? 4 : v->cap;
    while (newcap < needed) {
        if (newcap > SIZE_MAX / 2) {
            newcap = needed;
            break;
        }
        newcap *= 2;
    }
    if (newcap > SIZE_MAX / sizeof(LsString*)) {
        ls_rt_alloc_failure(newcap);
    }
    replacement = (LsString**)realloc(v->data, newcap * sizeof(LsString*));
    if (replacement == NULL) {
        ls_rt_alloc_failure(newcap * sizeof(LsString*));
    }
    v->data = replacement;
    v->cap = newcap;
}

/* Append an owned reference (ownership transfers to the vector). */
void ls_vec_string_push_owned(LsVec_String* v, LsString* value) {
    ls_vec_string_reserve_internal(v, v->len + 1);
    v->data[v->len] = value;
    v->len += 1;
}

LsVec_String ls_vec_string_new(void) {
    LsVec_String v;
    v.len = 0;
    v.cap = 0;
    v.data = NULL;
    return v;
}

LsVec_String ls_vec_string_with_capacity(size_t capacity) {
    LsVec_String v = ls_vec_string_new();
    if (capacity != 0) {
        ls_vec_string_reserve_internal(&v, capacity);
    }
    return v;
}

void ls_vec_string_push(LsVec_String* v, const LsString* value) {
    ls_vec_string_push_owned(v, (LsString*)ls_retain((void*)value));
}

void ls_vec_string_insert(LsVec_String* v, size_t index,
                          const LsString* value) {
    LsString* owned;

    if (index > v->len) {
        index = v->len;
    }
    owned = (LsString*)ls_retain((void*)value);
    ls_vec_string_reserve_internal(v, v->len + 1);
    memmove(v->data + index + 1, v->data + index,
            (v->len - index) * sizeof(LsString*));
    v->data[index] = owned;
    v->len += 1;
}

void ls_vec_string_extend(LsVec_String* v, const LsVec_String* other) {
    size_t count = other->len;
    size_t i;

    if (count == 0) {
        return;
    }
    if (count > SIZE_MAX - v->len) {
        ls_rt_alloc_failure(count);
    }
    if (v->data == other->data) {
        /* Self-extend: snapshot the element handles before the buffer moves,
         * then retain each one for the appended slots. */
        LsString** snapshot = (LsString**)malloc(count * sizeof(LsString*));
        if (snapshot == NULL) {
            ls_rt_alloc_failure(count * sizeof(LsString*));
        }
        memcpy(snapshot, other->data, count * sizeof(LsString*));
        ls_vec_string_reserve_internal(v, v->len + count);
        for (i = 0; i < count; i += 1) {
            v->data[v->len] = (LsString*)ls_retain((void*)snapshot[i]);
            v->len += 1;
        }
        free(snapshot);
        return;
    }
    ls_vec_string_reserve_internal(v, v->len + count);
    for (i = 0; i < count; i += 1) {
        v->data[v->len] = (LsString*)ls_retain((void*)other->data[i]);
        v->len += 1;
    }
}

LsString* ls_vec_string_remove(LsVec_String* v, size_t index) {
    LsString* removed;

    if (index >= v->len) {
        return NULL;
    }
    removed = v->data[index];
    memmove(v->data + index, v->data + index + 1,
            (v->len - index - 1) * sizeof(LsString*));
    v->len -= 1;
    return removed;
}

bool ls_vec_string_pop(LsVec_String* v, LsString** out) {
    if (v->len == 0) {
        return false;
    }
    v->len -= 1;
    *out = v->data[v->len];
    return true;
}

bool ls_vec_string_get(const LsVec_String* v, size_t index, LsString** out) {
    if (index >= v->len) {
        return false;
    }
    *out = (LsString*)ls_retain((void*)v->data[index]);
    return true;
}

void ls_vec_string_clear(LsVec_String* v) {
    size_t i;
    for (i = 0; i < v->len; i += 1) {
        ls_release(v->data[i]);
    }
    v->len = 0;
}

void ls_vec_string_reverse(LsVec_String* v) {
    size_t i;
    for (i = 0; i < v->len / 2; i += 1) {
        LsString* tmp = v->data[i];
        v->data[i] = v->data[v->len - 1 - i];
        v->data[v->len - 1 - i] = tmp;
    }
}

void ls_vec_string_free(LsVec_String* v) {
    size_t i;
    for (i = 0; i < v->len; i += 1) {
        ls_release(v->data[i]);
    }
    free(v->data);
    v->len = 0;
    v->cap = 0;
    v->data = NULL;
}

