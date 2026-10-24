/* lsaca C17 runtime implementation.
 *
 * Portable C17, no compiler extensions, no VLAs. See lsaca_rt.h for the ABI
 * contract and README.md ("Memory management (ORC)") for the ownership rules.
 */

#include "lsaca_rt.h"

#include <errno.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(LSACA_DEBUG) || defined(LSACA_DEBUG_RC)
#define LS_RT_LEAK_TRACKING 1
#endif

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

/* Rust programs write stdout/stderr as raw byte streams. The Windows C runtime
 * would translate '\n' into "\r\n" when a stream is in text mode, which would
 * make print/println output and panic reports platform dependent. Switch both
 * output streams to binary mode once so generated programs produce identical
 * bytes everywhere; stdin stays in text mode because lines are read with
 * fgets() and ls_read_line strips a trailing CR. */
static int ls_rt_stdio_configured = 0;

static void ls_rt_configure_stdio(void) {
    if (ls_rt_stdio_configured) {
        return;
    }
    ls_rt_stdio_configured = 1;
#ifdef _WIN32
    (void)_setmode(_fileno(stdout), _O_BINARY);
    (void)_setmode(_fileno(stderr), _O_BINARY);
#endif
}

/* ====================================================================== */
/* Reference counted allocations                                          */
/* ====================================================================== */

/* Header stored immediately before every payload. Keeping the leaked-object
 * list intrusive (inside the header) means leak tracking needs no extra
 * allocation. */
#define LS_RT_LIVE_MAGIC ((uint64_t)0x4C53414341525431ULL)

typedef struct LsRtHeader {
    size_t refcount;
    size_t size; /* payload size in bytes */
    uint64_t magic;
#ifdef LS_RT_LEAK_TRACKING
    struct LsRtHeader* prev;
    struct LsRtHeader* next;
#endif
} LsRtHeader;

/* Distance from the malloc block to the payload, rounded up so the payload is
 * aligned for every C type. */
#define LS_RT_ALIGN ((size_t)_Alignof(max_align_t))
#define LS_RT_HEADER_SPACE \
    (((sizeof(LsRtHeader) + LS_RT_ALIGN - 1) / LS_RT_ALIGN) * LS_RT_ALIGN)

static LsRtHeader* ls_rt_header_of(const void* payload) {
    return (LsRtHeader*)(void*)((char*)payload - LS_RT_HEADER_SPACE);
}

static void* ls_rt_payload_of(LsRtHeader* header) {
    return (void*)((char*)header + LS_RT_HEADER_SPACE);
}

#ifdef LS_RT_LEAK_TRACKING
static LsRtHeader* ls_rt_leak_head = NULL;
#endif

/* Every allocation failure is fatal; there is no recoverable OOM in the
 * language. Print a diagnostic and exit with the panic exit code. */
LS_NORETURN static void ls_rt_alloc_failure(size_t bytes) {
    ls_rt_configure_stdio();
    fflush(stdout);
    fprintf(stderr, "lsaca: memory allocation of %zu byte(s) failed\n", bytes);
    fflush(stderr);
    exit(101);
}

static void* ls_rt_alloc_payload(size_t size) {
    size_t total;
    LsRtHeader* header;
    void* payload;

    if (size > SIZE_MAX - LS_RT_HEADER_SPACE) {
        ls_rt_alloc_failure(size);
    }
    total = LS_RT_HEADER_SPACE + size;
    header = (LsRtHeader*)malloc(total);
    if (header == NULL) {
        ls_rt_alloc_failure(size);
    }
    header->refcount = 1;
    header->size = size;
    header->magic = LS_RT_LIVE_MAGIC;
#ifdef LS_RT_LEAK_TRACKING
    header->prev = NULL;
    header->next = ls_rt_leak_head;
    if (ls_rt_leak_head != NULL) {
        ls_rt_leak_head->prev = header;
    }
    ls_rt_leak_head = header;
#endif
    payload = ls_rt_payload_of(header);
    memset(payload, 0, size);
    return payload;
}

void* ls_alloc(size_t size) {
    return ls_rt_alloc_payload(size);
}

void* ls_retain(void* obj) {
    if (obj != NULL) {
        LsRtHeader* header = ls_rt_header_of(obj);
        header->refcount += 1;
    }
    return obj;
}

void ls_release(void* obj) {
    LsRtHeader* header;

    if (obj == NULL) {
        return;
    }
    header = ls_rt_header_of(obj);
    if (header->magic != LS_RT_LIVE_MAGIC || header->refcount == 0) {
        ls_rt_configure_stdio();
        fflush(stdout);
        fprintf(stderr,
                "lsaca: runtime error: release of an invalid or already "
                "released reference-counted object\n");
        fflush(stderr);
        exit(101);
    }
    header->refcount -= 1;
    if (header->refcount == 0) {
#ifdef LS_RT_LEAK_TRACKING
        if (header->prev != NULL) {
            header->prev->next = header->next;
        } else {
            ls_rt_leak_head = header->next;
        }
        if (header->next != NULL) {
            header->next->prev = header->prev;
        }
#endif
        header->magic = 0;
        free(header);
    }
}

size_t ls_refcount(const void* obj) {
    if (obj == NULL) {
        return 0;
    }
    return ls_rt_header_of(obj)->refcount;
}

/* ====================================================================== */
/* Runtime lifetime                                                       */
/* ====================================================================== */

static int ls_rt_argc = 0;
static char** ls_rt_argv = NULL;

int32_t ls_runtime_init(int argc, char** argv) {
    ls_rt_configure_stdio();
    ls_rt_argc = argc > 0 ? argc : 0;
    ls_rt_argv = argv;
    return 0;
}

void ls_runtime_shutdown(void) {
    ls_rt_configure_stdio();
    fflush(stdout);
#ifdef LS_RT_LEAK_TRACKING
    {
        size_t count = 0;
        size_t shown = 0;
        LsRtHeader* header;
        for (header = ls_rt_leak_head; header != NULL; header = header->next) {
            count += 1;
        }
        if (count != 0) {
            fprintf(stderr,
                    "lsaca: warning: %zu reference-counted allocation(s) "
                    "leaked\n",
                    count);
            for (header = ls_rt_leak_head;
                 header != NULL && shown < 8;
                 header = header->next, shown += 1) {
                fprintf(stderr, "lsaca:   leaked allocation of %zu bytes\n",
                        header->size);
            }
        }
        /* The process is about to exit; forget the list so that a second
         * shutdown call cannot report the same objects twice. */
        ls_rt_leak_head = NULL;
    }
#endif
    fflush(stderr);
}

/* ====================================================================== */
/* String object internals                                                */
/* ====================================================================== */

/* A String is exactly one reference counted allocation:
 *
 *     [LsString header { len, cap, data }][ capacity bytes ][ NUL ]
 *                                           ^ data
 *
 * so growing a string or detaching a shared string allocates a new object and
 * releases the old one; mutators return the new handle. */

static size_t ls_str_grown_capacity(size_t capacity, size_t needed) {
    size_t next = capacity < 8 ? 8 : capacity;
    while (next < needed) {
        if (next > SIZE_MAX / 2) {
            return needed;
        }
        next *= 2;
    }
    return next;
}

static LsString* ls_str_alloc_capacity(size_t capacity) {
    LsString* s;
    size_t payload_size;

    if (capacity > SIZE_MAX - sizeof(LsString) - 1) {
        ls_rt_alloc_failure(capacity);
    }
    payload_size = sizeof(LsString) + capacity + 1;
    s = (LsString*)ls_rt_alloc_payload(payload_size);
    s->len = 0;
    s->cap = capacity;
    s->data = (char*)((char*)s + sizeof(LsString));
    s->data[0] = '\0';
    return s;
}

static LsString* ls_str_alloc_from(const char* bytes, size_t len) {
    LsString* s = ls_str_alloc_capacity(len);
    if (len != 0 && bytes != NULL) {
        memcpy(s->data, bytes, len);
    }
    s->len = len;
    s->data[len] = '\0';
    return s;
}

/* Ensure room for len + additional bytes. Consumes the caller's reference to
 * "s" and returns the handle to keep: the same handle when it is unique and
 * has enough room, otherwise a freshly allocated (possibly detached) copy
 * with the old reference released. */
static LsString* ls_str_ensure_room(LsString* s, size_t additional) {
    size_t needed;
    size_t capacity;
    LsString* replacement;

    if (additional > SIZE_MAX - s->len - 1) {
        ls_rt_alloc_failure(additional);
    }
    needed = s->len + additional;
    if (ls_refcount(s) > 1) {
        capacity = ls_str_grown_capacity(s->cap, needed);
        replacement = ls_str_alloc_capacity(capacity);
        if (s->len != 0) {
            memcpy(replacement->data, s->data, s->len);
        }
        replacement->len = s->len;
        replacement->data[s->len] = '\0';
        ls_release(s);
        return replacement;
    }
    if (needed > s->cap) {
        capacity = ls_str_grown_capacity(s->cap, needed);
        replacement = ls_str_alloc_capacity(capacity);
        if (s->len != 0) {
            memcpy(replacement->data, s->data, s->len);
        }
        replacement->len = s->len;
        replacement->data[s->len] = '\0';
        ls_release(s);
        return replacement;
    }
    return s;
}

static LsString* ls_str_append_bytes(LsString* s, const char* bytes,
                                     size_t count) {
    s = ls_str_ensure_room(s, count);
    if (count != 0 && bytes != NULL) {
        memcpy(s->data + s->len, bytes, count);
    }
    s->len += count;
    s->data[s->len] = '\0';
    return s;
}

/* ====================================================================== */
/* String constructors and observers                                      */
/* ====================================================================== */

LsString* ls_str_new(void) {
    return ls_str_alloc_capacity(0);
}

LsString* ls_str_cstr(const char* cstr) {
    if (cstr == NULL) {
        return ls_str_alloc_capacity(0);
    }
    return ls_str_alloc_from(cstr, strlen(cstr));
}

LsString* ls_str_from_parts(const char* bytes, size_t len) {
    if (bytes == NULL || len == 0) {
        return ls_str_alloc_capacity(0);
    }
    return ls_str_alloc_from(bytes, len);
}

LsString* ls_str_with_capacity(size_t capacity) {
    return ls_str_alloc_capacity(capacity);
}

size_t ls_str_len(const LsString* s) {
    return s->len;
}

bool ls_str_is_empty(const LsString* s) {
    return s->len == 0;
}

const char* ls_str_data(const LsString* s) {
    return s->data;
}

LsString* ls_str_clone(const LsString* s) {
    return ls_str_alloc_from(s->data, s->len);
}

bool ls_str_eq(const LsString* a, const LsString* b) {
    if (a->len != b->len) {
        return false;
    }
    if (a->len == 0) {
        return true;
    }
    return memcmp(a->data, b->data, a->len) == 0;
}

int32_t ls_str_cmp(const LsString* a, const LsString* b) {
    size_t common = a->len < b->len ? a->len : b->len;
    int order = common != 0 ? memcmp(a->data, b->data, common) : 0;
    if (order != 0) {
        return order < 0 ? -1 : 1;
    }
    if (a->len == b->len) {
        return 0;
    }
    return a->len < b->len ? -1 : 1;
}

LsString* ls_str_concat(const LsString* a, const LsString* b) {
    size_t total;
    LsString* result;

    if (a->len > SIZE_MAX - b->len) {
        ls_rt_alloc_failure(a->len);
    }
    total = a->len + b->len;
    result = ls_str_alloc_capacity(total);
    if (a->len != 0) {
        memcpy(result->data, a->data, a->len);
    }
    if (b->len != 0) {
        memcpy(result->data + a->len, b->data, b->len);
    }
    result->len = total;
    result->data[total] = '\0';
    return result;
}

/* ====================================================================== */
/* String mutation (copy on write, consumes the receiver reference)       */
/* ====================================================================== */

LsString* ls_str_push_str(LsString* s, const LsString* suffix) {
    size_t extra;

    if (s == suffix) {
        extra = s->len;
        s = ls_str_ensure_room(s, extra);
        if (extra != 0) {
            memcpy(s->data + s->len, s->data, extra);
        }
        s->len += extra;
        s->data[s->len] = '\0';
        return s;
    }
    extra = suffix->len;
    return ls_str_append_bytes(s, suffix->data, extra);
}

LsString* ls_str_push_cstr(LsString* s, const char* suffix) {
    if (suffix == NULL) {
        return s;
    }
    return ls_str_append_bytes(s, suffix, strlen(suffix));
}

LsString* ls_str_push_char(LsString* s, char c) {
    return ls_str_append_bytes(s, &c, 1);
}

LsString* ls_str_clear(LsString* s) {
    if (ls_refcount(s) > 1) {
        LsString* replacement = ls_str_alloc_capacity(s->cap);
        ls_release(s);
        return replacement;
    }
    s->len = 0;
    s->data[0] = '\0';
    return s;
}

LsString* ls_str_reserve(LsString* s, size_t additional) {
    return ls_str_ensure_room(s, additional);
}

/* ====================================================================== */
/* String search and transformation                                       */
/* ====================================================================== */

int64_t ls_str_find(const LsString* haystack, const LsString* needle) {
    size_t last;
    size_t i;

    if (needle->len == 0) {
        return 0;
    }
    if (needle->len > haystack->len) {
        return -1;
    }
    last = haystack->len - needle->len;
    for (i = 0; i <= last; i += 1) {
        if (memcmp(haystack->data + i, needle->data, needle->len) == 0) {
            return (int64_t)i;
        }
    }
    return -1;
}

bool ls_str_contains(const LsString* haystack, const LsString* needle) {
    return ls_str_find(haystack, needle) >= 0;
}

bool ls_str_starts_with(const LsString* s, const LsString* prefix) {
    if (prefix->len > s->len) {
        return false;
    }
    return prefix->len == 0 ||
           memcmp(s->data, prefix->data, prefix->len) == 0;
}

bool ls_str_ends_with(const LsString* s, const LsString* suffix) {
    if (suffix->len > s->len) {
        return false;
    }
    return suffix->len == 0 ||
           memcmp(s->data + (s->len - suffix->len), suffix->data,
                  suffix->len) == 0;
}

static size_t ls_str_count_occurrences(const LsString* s,
                                       const LsString* needle) {
    size_t count = 0;
    size_t i = 0;

    while (i + needle->len <= s->len) {
        if (memcmp(s->data + i, needle->data, needle->len) == 0) {
            count += 1;
            i += needle->len;
        } else {
            i += 1;
        }
    }
    return count;
}

LsString* ls_str_replace(const LsString* s, const LsString* from,
                         const LsString* to) {
    size_t count;
    size_t total;
    size_t in;
    size_t out;
    LsString* result;

    if (from->len == 0) {
        count = s->len + 1;
    } else {
        count = ls_str_count_occurrences(s, from);
    }
    if (to->len >= from->len) {
        size_t growth = to->len - from->len;
        if (growth != 0 && count > (SIZE_MAX - s->len) / growth) {
            ls_rt_alloc_failure(s->len);
        }
        total = s->len + count * growth;
    } else {
        size_t shrink = from->len - to->len;
        total = s->len - count * shrink;
    }
    result = ls_str_alloc_capacity(total);
    out = 0;
    if (from->len == 0) {
        if (to->len != 0) {
            memcpy(result->data + out, to->data, to->len);
        }
        out += to->len;
        for (in = 0; in < s->len; in += 1) {
            result->data[out] = s->data[in];
            out += 1;
            if (to->len != 0) {
                memcpy(result->data + out, to->data, to->len);
            }
            out += to->len;
        }
    } else {
        in = 0;
        while (in < s->len) {
            if (in + from->len <= s->len &&
                memcmp(s->data + in, from->data, from->len) == 0) {
                if (to->len != 0) {
                    memcpy(result->data + out, to->data, to->len);
                }
                out += to->len;
                in += from->len;
            } else {
                result->data[out] = s->data[in];
                out += 1;
                in += 1;
            }
        }
    }
    result->len = out;
    result->data[out] = '\0';
    return result;
}

LsString* ls_str_to_uppercase(const LsString* s) {
    LsString* result = ls_str_alloc_capacity(s->len);
    size_t i;
    for (i = 0; i < s->len; i += 1) {
        char c = s->data[i];
        if (c >= 'a' && c <= 'z') {
            c = (char)(c - 'a' + 'A');
        }
        result->data[i] = c;
    }
    result->len = s->len;
    result->data[s->len] = '\0';
    return result;
}

LsString* ls_str_to_lowercase(const LsString* s) {
    LsString* result = ls_str_alloc_capacity(s->len);
    size_t i;
    for (i = 0; i < s->len; i += 1) {
        char c = s->data[i];
        if (c >= 'A' && c <= 'Z') {
            c = (char)(c - 'A' + 'a');
        }
        result->data[i] = c;
    }
    result->len = s->len;
    result->data[s->len] = '\0';
    return result;
}

static bool ls_rt_is_ascii_whitespace(unsigned char c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == 0x0B ||
           c == 0x0C;
}

LsString* ls_str_trim(const LsString* s) {
    size_t start = 0;
    size_t end = s->len;
    while (start < end &&
           ls_rt_is_ascii_whitespace((unsigned char)s->data[start])) {
        start += 1;
    }
    while (end > start &&
           ls_rt_is_ascii_whitespace((unsigned char)s->data[end - 1])) {
        end -= 1;
    }
    return ls_str_alloc_from(s->data + start, end - start);
}

LsString* ls_str_substring(const LsString* s, size_t start, size_t len,
                           const char* file, int32_t line, int32_t col) {
    if (start > s->len) {
        ls_panic_bounds(file, line, col, (int64_t)s->len, (int64_t)start);
    }
    if (len > s->len - start) {
        ls_panic_bounds(file, line, col, (int64_t)s->len,
                        (int64_t)(start + len));
    }
    return ls_str_alloc_from(s->data + start, len);
}

LsString* ls_str_repeat(const LsString* s, size_t count, const char* file,
                        int32_t line, int32_t col) {
    LsString* result;
    size_t total;
    size_t i;

    if (count == 0 || s->len == 0) {
        return ls_str_alloc_capacity(0);
    }
    if (count > SIZE_MAX / s->len) {
        ls_panic_cstr(file, line, col, "capacity overflow");
    }
    total = count * s->len;
    result = ls_str_alloc_capacity(total);
    for (i = 0; i < count; i += 1) {
        memcpy(result->data + i * s->len, s->data, s->len);
    }
    result->len = total;
    result->data[total] = '\0';
    return result;
}

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
    char* base;
    char* temp;

    if (elem_size == 0) {
        *len += 1;
        return;
    }
    if (index > *len) {
        index = *len;
    }
    temp = (char*)malloc(elem_size);
    if (temp == NULL) {
        ls_rt_alloc_failure(elem_size);
    }
    memcpy(temp, value, elem_size);
    ls_vec_grow(data, cap, *len, elem_size);
    base = (char*)*data;
    memmove(base + (index + 1) * elem_size, base + index * elem_size,
            (*len - index) * elem_size);
    memcpy(base + index * elem_size, temp, elem_size);
    free(temp);
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
static void ls_vec_string_push_owned(LsVec_String* v, LsString* value) {
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

/* ====================================================================== */
/* String split (needs LsVec_String)                                      */
/* ====================================================================== */

/* Length of one UTF-8 scalar value at p (at least 1); malformed sequences are
 * treated as single bytes so split never fails. */
static size_t ls_rt_utf8_sequence_len(const unsigned char* p, size_t remaining) {
    size_t n;
    size_t i;

    if (remaining == 0) {
        return 0;
    }
    if (p[0] < 0x80) {
        return 1;
    }
    if ((p[0] & 0xE0) == 0xC0) {
        n = 2;
    } else if ((p[0] & 0xF0) == 0xE0) {
        n = 3;
    } else if ((p[0] & 0xF8) == 0xF0) {
        n = 4;
    } else {
        return 1;
    }
    if (n > remaining) {
        return 1;
    }
    for (i = 1; i < n; i += 1) {
        if ((p[i] & 0xC0) != 0x80) {
            return 1;
        }
    }
    return n;
}

LsVec_String ls_str_split(const LsString* s, const LsString* separator) {
    LsVec_String out = ls_vec_string_new();

    if (separator->len == 0) {
        size_t i = 0;
        ls_vec_string_push_owned(&out, ls_str_new());
        while (i < s->len) {
            size_t n = ls_rt_utf8_sequence_len(
                (const unsigned char*)s->data + i, s->len - i);
            ls_vec_string_push_owned(&out, ls_str_alloc_from(s->data + i, n));
            i += n;
        }
        ls_vec_string_push_owned(&out, ls_str_new());
        return out;
    }
    {
        size_t start = 0;
        size_t i = 0;
        while (i + separator->len <= s->len) {
            if (memcmp(s->data + i, separator->data, separator->len) == 0) {
                ls_vec_string_push_owned(
                    &out, ls_str_alloc_from(s->data + start, i - start));
                i += separator->len;
                start = i;
            } else {
                i += 1;
            }
        }
        ls_vec_string_push_owned(
            &out, ls_str_alloc_from(s->data + start, s->len - start));
    }
    return out;
}

/* ====================================================================== */
/* String parsing                                                         */
/* ====================================================================== */

bool ls_str_parse_i64(const LsString* s, int64_t* out) {
    size_t i = 0;
    bool negative = false;
    uint64_t acc = 0;
    uint64_t limit;

    if (s->len == 0) {
        return false;
    }
    if (s->data[0] == '+' || s->data[0] == '-') {
        negative = s->data[0] == '-';
        i = 1;
        if (i == s->len) {
            return false;
        }
    }
    for (; i < s->len; i += 1) {
        unsigned char c = (unsigned char)s->data[i];
        unsigned digit;
        if (c < '0' || c > '9') {
            return false;
        }
        digit = (unsigned)(c - '0');
        if (acc > (UINT64_MAX - digit) / 10) {
            return false;
        }
        acc = acc * 10 + digit;
    }
    limit = negative ? ((uint64_t)INT64_MAX + 1) : (uint64_t)INT64_MAX;
    if (acc > limit) {
        return false;
    }
    if (negative) {
        if (acc == (uint64_t)INT64_MAX + 1) {
            *out = INT64_MIN;
        } else {
            *out = -(int64_t)acc;
        }
    } else {
        *out = (int64_t)acc;
    }
    return true;
}

/* Case-insensitive match of "len" bytes against an ASCII lowercase word. */
static bool ls_rt_match_word(const char* text, size_t len, const char* word) {
    size_t i;
    if (strlen(word) != len) {
        return false;
    }
    for (i = 0; i < len; i += 1) {
        char c = text[i];
        if (c >= 'A' && c <= 'Z') {
            c = (char)(c - 'A' + 'a');
        }
        if (c != word[i]) {
            return false;
        }
    }
    return true;
}

bool ls_str_parse_f64(const LsString* s, double* out) {
    size_t i = 0;
    bool have_int_digits = false;
    bool have_frac_digits = false;
    bool negative = false;

    if (s->len == 0) {
        return false;
    }
    if (s->data[0] == '+' || s->data[0] == '-') {
        negative = s->data[0] == '-';
        i = 1;
    }
    if (i >= s->len) {
        return false;
    }
    if (ls_rt_match_word(s->data + i, s->len - i, "infinity") ||
        ls_rt_match_word(s->data + i, s->len - i, "inf")) {
        *out = negative ? -INFINITY : INFINITY;
        return true;
    }
    if (ls_rt_match_word(s->data + i, s->len - i, "nan")) {
        *out = NAN;
        return true;
    }
    while (i < s->len && s->data[i] >= '0' && s->data[i] <= '9') {
        i += 1;
        have_int_digits = true;
    }
    if (i < s->len && s->data[i] == '.') {
        i += 1;
        while (i < s->len && s->data[i] >= '0' && s->data[i] <= '9') {
            i += 1;
            have_frac_digits = true;
        }
    }
    if (!have_int_digits && !have_frac_digits) {
        return false;
    }
    if (i < s->len && (s->data[i] == 'e' || s->data[i] == 'E')) {
        bool have_exp_digits = false;
        i += 1;
        if (i < s->len && (s->data[i] == '+' || s->data[i] == '-')) {
            i += 1;
        }
        while (i < s->len && s->data[i] >= '0' && s->data[i] <= '9') {
            i += 1;
            have_exp_digits = true;
        }
        if (!have_exp_digits) {
            return false;
        }
    }
    if (i != s->len) {
        return false;
    }
    *out = strtod(s->data, NULL);
    return true;
}

/* ====================================================================== */
/* Formatting: builder primitives                                         */
/* ====================================================================== */

LsString* ls_format_begin(size_t capacity_hint) {
    return ls_str_alloc_capacity(capacity_hint);
}

static LsString* ls_fmt_append(LsString* acc, const char* bytes, size_t len) {
    return ls_str_append_bytes(acc, bytes, len);
}

LsString* ls_format_push_str(LsString* acc, const LsString* piece) {
    if (acc == piece) {
        LsString* copy = ls_str_clone(piece);
        acc = ls_fmt_append(acc, copy->data, copy->len);
        ls_release(copy);
        return acc;
    }
    return ls_fmt_append(acc, piece->data, piece->len);
}

LsString* ls_format_push_char(LsString* acc, char c) {
    return ls_fmt_append(acc, &c, 1);
}

LsString* ls_format_push_cstr(LsString* acc, const char* cstr) {
    if (cstr == NULL) {
        return acc;
    }
    return ls_fmt_append(acc, cstr, strlen(cstr));
}

LsString* ls_fmt_i64(LsString* acc, int64_t v) {
    char buf[32];
    int written = snprintf(buf, sizeof buf, "%" PRId64, v);
    if (written < 0) {
        return acc;
    }
    return ls_fmt_append(acc, buf, (size_t)written);
}

LsString* ls_fmt_u64(LsString* acc, uint64_t v) {
    char buf[32];
    int written = snprintf(buf, sizeof buf, "%" PRIu64, v);
    if (written < 0) {
        return acc;
    }
    return ls_fmt_append(acc, buf, (size_t)written);
}

LsString* ls_fmt_bool(LsString* acc, bool v) {
    return ls_fmt_append(acc, v ? "true" : "false", v ? 4 : 5);
}

/* ====================================================================== */
/* Formatting: shortest round-trip floating point                         */
/* ====================================================================== */

/* The shortest decimal representation is found by printing the value with
 * 1, 2, ... significant digits and parsing it back until the bit pattern
 * matches. That is the classic brute-force definition of "shortest round-trip"
 * and needs no big-integer code. */

static bool ls_rt_f64_same_bits(double a, double b) {
    uint64_t x;
    uint64_t y;
    memcpy(&x, &a, sizeof x);
    memcpy(&y, &b, sizeof y);
    return x == y;
}

static bool ls_rt_f32_same_bits(float a, float b) {
    uint32_t x;
    uint32_t y;
    memcpy(&x, &a, sizeof x);
    memcpy(&y, &b, sizeof y);
    return x == y;
}

/* Parse "d.dddde±XX" produced by printf("%.*e") into significant digits and a
 * decimal exponent (value = 0.digits * 10^(exponent+1)). */
static void ls_rt_parse_scientific(const char* buf, char* digits,
                                   size_t digits_cap, size_t* out_ndigits,
                                   int* out_exponent) {
    const char* p = buf;
    size_t nd = 0;

    if (*p == '+' || *p == '-') {
        p += 1;
    }
    while (*p != '\0' && *p != 'e' && *p != 'E') {
        if (*p >= '0' && *p <= '9' && nd + 1 < digits_cap) {
            digits[nd] = *p;
            nd += 1;
        }
        p += 1;
    }
    while (nd > 1 && digits[nd - 1] == '0') {
        nd -= 1;
    }
    *out_ndigits = nd;
    digits[nd] = '\0';
    if (*p == 'e' || *p == 'E') {
        p += 1;
    }
    *out_exponent = (int)strtol(p, NULL, 10);
}

static void ls_rt_shortest_f64(double value, char* digits, size_t* out_ndigits,
                               int* out_exponent) {
    char buf[64];
    int precision;
    bool found = false;

    for (precision = 1; precision <= 17; precision += 1) {
        int written = snprintf(buf, sizeof buf, "%.*e", precision - 1, value);
        if (written < 0) {
            break;
        }
        if (ls_rt_f64_same_bits(strtod(buf, NULL), value)) {
            found = true;
            break;
        }
    }
    if (!found) {
        (void)snprintf(buf, sizeof buf, "%.17e", value);
    }
    ls_rt_parse_scientific(buf, digits, 24, out_ndigits, out_exponent);
}

static void ls_rt_shortest_f32(float value, char* digits, size_t* out_ndigits,
                               int* out_exponent) {
    char buf[64];
    int precision;
    bool found = false;

    for (precision = 1; precision <= 9; precision += 1) {
        int written =
            snprintf(buf, sizeof buf, "%.*e", precision - 1, (double)value);
        if (written < 0) {
            break;
        }
        if (ls_rt_f32_same_bits(strtof(buf, NULL), value)) {
            found = true;
            break;
        }
    }
    if (!found) {
        (void)snprintf(buf, sizeof buf, "%.9e", (double)value);
    }
    ls_rt_parse_scientific(buf, digits, 24, out_ndigits, out_exponent);
}

/* Render significant digits as Rust's Display (decimal expansion, no
 * exponent) or Debug (decimal for 1e-4 <= |v| < 1e16, exponent otherwise,
 * integral values get a trailing ".0"). */
static LsString* ls_rt_render_float(LsString* acc, bool negative, bool debug,
                                    const char* digits, size_t ndigits,
                                    int exponent) {
    size_t i;

    if (negative) {
        acc = ls_fmt_append(acc, "-", 1);
    }
    if (debug && (exponent < -4 || exponent >= 16)) {
        acc = ls_fmt_append(acc, digits, 1);
        if (ndigits > 1) {
            acc = ls_fmt_append(acc, ".", 1);
            acc = ls_fmt_append(acc, digits + 1, ndigits - 1);
        }
        acc = ls_fmt_append(acc, "e", 1);
        if (exponent < 0) {
            acc = ls_fmt_append(acc, "-", 1);
        }
        {
            int magnitude = exponent < 0 ? -exponent : exponent;
            char buf[16];
            int written = snprintf(buf, sizeof buf, "%d", magnitude);
            if (written > 0) {
                acc = ls_fmt_append(acc, buf, (size_t)written);
            }
        }
        return acc;
    }
    if (exponent >= 0) {
        size_t integer_digits = (size_t)exponent + 1;
        for (i = 0; i < integer_digits; i += 1) {
            char c = i < ndigits ? digits[i] : '0';
            acc = ls_fmt_append(acc, &c, 1);
        }
        if (debug) {
            acc = ls_fmt_append(acc, ".", 1);
            if (ndigits > integer_digits) {
                acc = ls_fmt_append(acc, digits + integer_digits,
                                    ndigits - integer_digits);
            } else {
                acc = ls_fmt_append(acc, "0", 1);
            }
        } else if (ndigits > integer_digits) {
            acc = ls_fmt_append(acc, ".", 1);
            acc = ls_fmt_append(acc, digits + integer_digits,
                                ndigits - integer_digits);
        }
        return acc;
    }
    acc = ls_fmt_append(acc, "0.", 2);
    for (i = 0; i < (size_t)(-exponent - 1); i += 1) {
        acc = ls_fmt_append(acc, "0", 1);
    }
    acc = ls_fmt_append(acc, digits, ndigits);
    return acc;
}

static LsString* ls_rt_format_float(LsString* acc, bool debug, double value,
                                    bool is_f32) {
    bool negative;
    char digits[24];
    size_t ndigits = 0;
    int exponent = 0;

    if (isnan(value)) {
        return ls_fmt_append(acc, "NaN", 3);
    }
    if (isinf(value)) {
        if (value < 0) {
            acc = ls_fmt_append(acc, "-", 1);
        }
        return ls_fmt_append(acc, "inf", 3);
    }
    negative = value < 0 || (value == 0 && signbit(value) != 0);
    if (value == 0) {
        if (negative) {
            acc = ls_fmt_append(acc, "-", 1);
        }
        return ls_fmt_append(acc, debug ? "0.0" : "0", debug ? 3 : 1);
    }
    if (is_f32) {
        ls_rt_shortest_f32((float)value, digits, &ndigits, &exponent);
    } else {
        ls_rt_shortest_f64(value, digits, &ndigits, &exponent);
    }
    return ls_rt_render_float(acc, negative, debug, digits, ndigits, exponent);
}

LsString* ls_fmt_f64(LsString* acc, double v) {
    return ls_rt_format_float(acc, false, v, false);
}

LsString* ls_fmt_f32(LsString* acc, float v) {
    return ls_rt_format_float(acc, false, (double)v, true);
}

LsString* ls_fmt_f64_debug(LsString* acc, double v) {
    return ls_rt_format_float(acc, true, v, false);
}

LsString* ls_fmt_f32_debug(LsString* acc, float v) {
    return ls_rt_format_float(acc, true, (double)v, true);
}

/* Rust-style Debug quoting for strings: escape quote, backslash and the
 * common control characters; other control bytes use \u{...}. */
LsString* ls_fmt_debug_string(LsString* acc, const LsString* v) {
    size_t i;

    acc = ls_fmt_append(acc, "\"", 1);
    for (i = 0; i < v->len; i += 1) {
        unsigned char c = (unsigned char)v->data[i];
        switch (c) {
        case '"':
            acc = ls_fmt_append(acc, "\\\"", 2);
            break;
        case '\\':
            acc = ls_fmt_append(acc, "\\\\", 2);
            break;
        case '\n':
            acc = ls_fmt_append(acc, "\\n", 2);
            break;
        case '\r':
            acc = ls_fmt_append(acc, "\\r", 2);
            break;
        case '\t':
            acc = ls_fmt_append(acc, "\\t", 2);
            break;
        default:
            if (c < 0x20 || c == 0x7F) {
                char buf[16];
                int written = snprintf(buf, sizeof buf, "\\u{%x}", (unsigned)c);
                if (written > 0) {
                    acc = ls_fmt_append(acc, buf, (size_t)written);
                }
            } else {
                char byte = (char)c;
                acc = ls_fmt_append(acc, &byte, 1);
            }
            break;
        }
    }
    return ls_fmt_append(acc, "\"", 1);
}

/* ====================================================================== */
/* Output                                                                 */
/* ====================================================================== */

void ls_print(const LsString* s) {
    ls_rt_configure_stdio();
    if (s != NULL && s->len != 0) {
        (void)fwrite(s->data, 1, s->len, stdout);
    }
    fflush(stdout);
}

void ls_println(const LsString* s) {
    ls_rt_configure_stdio();
    if (s != NULL && s->len != 0) {
        (void)fwrite(s->data, 1, s->len, stdout);
    }
    (void)fputc('\n', stdout);
    fflush(stdout);
}

void ls_eprint(const LsString* s) {
    ls_rt_configure_stdio();
    if (s != NULL && s->len != 0) {
        (void)fwrite(s->data, 1, s->len, stderr);
    }
    fflush(stderr);
}

void ls_eprintln(const LsString* s) {
    ls_rt_configure_stdio();
    if (s != NULL && s->len != 0) {
        (void)fwrite(s->data, 1, s->len, stderr);
    }
    (void)fputc('\n', stderr);
    fflush(stderr);
}

void ls_print_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stdout);
    }
    fflush(stdout);
}

void ls_println_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stdout);
    }
    (void)fputc('\n', stdout);
    fflush(stdout);
}

void ls_eprint_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stderr);
    }
    fflush(stderr);
}

void ls_eprintln_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stderr);
    }
    (void)fputc('\n', stderr);
    fflush(stderr);
}

void ls_print_flush(void) {
    ls_rt_configure_stdio();
    fflush(stdout);
    fflush(stderr);
}

/* ====================================================================== */
/* Panics                                                                 */
/* ====================================================================== */

/* Print the Rust-style panic report and exit(101). file may be NULL/empty. */
LS_NORETURN static void ls_rt_panic_report(const char* file, int32_t line,
                                           int32_t col, const char* msg,
                                           size_t msg_len) {
    const char* where = (file != NULL && file[0] != '\0') ? file : "<unknown>";
    ls_rt_configure_stdio();
    fflush(stdout);
    fprintf(stderr, "thread 'main' panicked at %s:%" PRId32 ":%" PRId32 ":\n",
            where, line, col);
    if (msg_len != 0) {
        (void)fwrite(msg, 1, msg_len, stderr);
    }
    (void)fputc('\n', stderr);
    fflush(stdout);
    fflush(stderr);
    exit(101);
}

LS_NORETURN static void ls_rt_panic_rem_zero(const char* file, int32_t line,
                                             int32_t col) {
    static const char message[] =
        "attempt to calculate the remainder with a divisor of zero";
    ls_rt_panic_report(file, line, col, message, sizeof message - 1);
}

LS_NORETURN static void ls_rt_panic_rem_overflow(const char* file, int32_t line,
                                                 int32_t col) {
    static const char message[] =
        "attempt to calculate the remainder with overflow";
    ls_rt_panic_report(file, line, col, message, sizeof message - 1);
}

void ls_panic_at(const char* file, int32_t line, int32_t col,
                 const LsString* msg) {
    if (msg == NULL) {
        ls_rt_panic_report(file, line, col, NULL, 0);
    }
    ls_rt_panic_report(file, line, col, msg->data, msg->len);
}

void ls_panic_cstr(const char* file, int32_t line, int32_t col,
                   const char* msg) {
    if (msg == NULL) {
        ls_rt_panic_report(file, line, col, NULL, 0);
    }
    ls_rt_panic_report(file, line, col, msg, strlen(msg));
}

void ls_panic_bounds(const char* file, int32_t line, int32_t col, int64_t len,
                     int64_t index) {
    char message[160];
    int written = snprintf(message, sizeof message,
                           "index out of bounds: the len is %" PRId64
                           " but the index is %" PRId64,
                           len, index);
    size_t msg_len;
    if (written < 0) {
        written = 0;
    }
    msg_len = (size_t)written < sizeof message ? (size_t)written
                                               : sizeof message - 1;
    ls_rt_panic_report(file, line, col, message, msg_len);
}

void ls_panic_div_zero(const char* file, int32_t line, int32_t col) {
    static const char message[] = "attempt to divide by zero";
    ls_rt_panic_report(file, line, col, message, sizeof message - 1);
}

void ls_panic_overflow(const char* file, int32_t line, int32_t col,
                       const char* op) {
    char message[96];
    int written;
    size_t msg_len;
    if (op == NULL) {
        op = "compute";
    }
    written = snprintf(message, sizeof message, "attempt to %s with overflow",
                       op);
    if (written < 0) {
        written = 0;
    }
    msg_len = (size_t)written < sizeof message ? (size_t)written
                                               : sizeof message - 1;
    ls_rt_panic_report(file, line, col, message, msg_len);
}

void ls_exit(int32_t code) {
    ls_rt_configure_stdio();
    fflush(stdout);
    fflush(stderr);
    exit(code);
}

/* ====================================================================== */
/* Checked integer arithmetic                                             */
/* ====================================================================== */

#define LS_RT_BITS(ctype) ((int64_t)(sizeof(ctype) * 8))

/* ---------- 64-bit signed (portable overflow detection by hand) ---------- */

static bool ls_rt_i64_add_overflow(int64_t a, int64_t b, int64_t* out) {
    if (b > 0 && a > INT64_MAX - b) {
        return true;
    }
    if (b < 0 && a < INT64_MIN - b) {
        return true;
    }
    *out = a + b;
    return false;
}

static bool ls_rt_i64_sub_overflow(int64_t a, int64_t b, int64_t* out) {
    if (b > 0 && a < INT64_MIN + b) {
        return true;
    }
    if (b < 0 && a > INT64_MAX + b) {
        return true;
    }
    *out = a - b;
    return false;
}

static bool ls_rt_i64_mul_overflow(int64_t a, int64_t b, int64_t* out) {
    if (a == 0 || b == 0) {
        *out = 0;
        return false;
    }
    if (a == -1) {
        if (b == INT64_MIN) {
            return true;
        }
        *out = -b;
        return false;
    }
    if (b == -1) {
        if (a == INT64_MIN) {
            return true;
        }
        *out = -a;
        return false;
    }
    if (a > 0) {
        if (b > 0) {
            if (a > INT64_MAX / b) {
                return true;
            }
        } else {
            if (b < INT64_MIN / a) {
                return true;
            }
        }
    } else {
        if (b > 0) {
            if (a < INT64_MIN / b) {
                return true;
            }
        } else {
            if (a < INT64_MAX / b) {
                return true;
            }
        }
    }
    *out = a * b;
    return false;
}

int64_t ls_add_i64_checked(int64_t a, int64_t b, const char* file, int32_t line,
                           int32_t col) {
    int64_t result;
    if (ls_rt_i64_add_overflow(a, b, &result)) {
        ls_panic_overflow(file, line, col, "add");
    }
    return result;
}

int64_t ls_sub_i64_checked(int64_t a, int64_t b, const char* file, int32_t line,
                           int32_t col) {
    int64_t result;
    if (ls_rt_i64_sub_overflow(a, b, &result)) {
        ls_panic_overflow(file, line, col, "subtract");
    }
    return result;
}

int64_t ls_mul_i64_checked(int64_t a, int64_t b, const char* file, int32_t line,
                           int32_t col) {
    int64_t result;
    if (ls_rt_i64_mul_overflow(a, b, &result)) {
        ls_panic_overflow(file, line, col, "multiply");
    }
    return result;
}

int64_t ls_div_i64_checked(int64_t a, int64_t b, const char* file, int32_t line,
                           int32_t col) {
    if (b == 0) {
        ls_panic_div_zero(file, line, col);
    }
    if (a == INT64_MIN && b == -1) {
        ls_panic_overflow(file, line, col, "divide");
    }
    return a / b;
}

int64_t ls_rem_i64_checked(int64_t a, int64_t b, const char* file, int32_t line,
                           int32_t col) {
    if (b == 0) {
        ls_rt_panic_rem_zero(file, line, col);
    }
    if (a == INT64_MIN && b == -1) {
        ls_rt_panic_rem_overflow(file, line, col);
    }
    return a % b;
}

int64_t ls_shl_i64_checked(int64_t a, int64_t b, const char* file, int32_t line,
                           int32_t col) {
    if (b < 0 || b >= LS_RT_BITS(int64_t)) {
        ls_panic_overflow(file, line, col, "shift left");
    }
    return (int64_t)((uint64_t)a << (unsigned)b);
}

int64_t ls_shr_i64_checked(int64_t a, int64_t b, const char* file, int32_t line,
                           int32_t col) {
    if (b < 0 || b >= LS_RT_BITS(int64_t)) {
        ls_panic_overflow(file, line, col, "shift right");
    }
    return a >> (unsigned)b;
}

int64_t ls_neg_i64_checked(int64_t a, const char* file, int32_t line,
                           int32_t col) {
    if (a == INT64_MIN) {
        ls_panic_overflow(file, line, col, "negate");
    }
    return -a;
}

/* ---------- 64-bit unsigned ---------- */

uint64_t ls_add_u64_checked(uint64_t a, uint64_t b, const char* file,
                            int32_t line, int32_t col) {
    if (a > UINT64_MAX - b) {
        ls_panic_overflow(file, line, col, "add");
    }
    return a + b;
}

uint64_t ls_sub_u64_checked(uint64_t a, uint64_t b, const char* file,
                            int32_t line, int32_t col) {
    if (a < b) {
        ls_panic_overflow(file, line, col, "subtract");
    }
    return a - b;
}

uint64_t ls_mul_u64_checked(uint64_t a, uint64_t b, const char* file,
                            int32_t line, int32_t col) {
    if (b != 0 && a > UINT64_MAX / b) {
        ls_panic_overflow(file, line, col, "multiply");
    }
    return a * b;
}

uint64_t ls_div_u64_checked(uint64_t a, uint64_t b, const char* file,
                            int32_t line, int32_t col) {
    if (b == 0) {
        ls_panic_div_zero(file, line, col);
    }
    return a / b;
}

uint64_t ls_rem_u64_checked(uint64_t a, uint64_t b, const char* file,
                            int32_t line, int32_t col) {
    if (b == 0) {
        ls_rt_panic_rem_zero(file, line, col);
    }
    return a % b;
}

uint64_t ls_shl_u64_checked(uint64_t a, int64_t b, const char* file,
                            int32_t line, int32_t col) {
    if (b < 0 || b >= LS_RT_BITS(uint64_t)) {
        ls_panic_overflow(file, line, col, "shift left");
    }
    return a << (unsigned)b;
}

uint64_t ls_shr_u64_checked(uint64_t a, int64_t b, const char* file,
                            int32_t line, int32_t col) {
    if (b < 0 || b >= LS_RT_BITS(uint64_t)) {
        ls_panic_overflow(file, line, col, "shift right");
    }
    return a >> (unsigned)b;
}

/* ---------- smaller signed types: widening arithmetic ---------- */

#define LS_RT_DEF_CHECKED_SIGNED_SMALL(sfx, ctype, widen, utype, minv, maxv)   \
    ctype ls_add_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        widen result = (widen)a + (widen)b;                                    \
        if (result < (widen)(minv) || result > (widen)(maxv)) {                \
            ls_panic_overflow(file, line, col, "add");                         \
        }                                                                      \
        return (ctype)result;                                                  \
    }                                                                          \
    ctype ls_sub_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        widen result = (widen)a - (widen)b;                                    \
        if (result < (widen)(minv) || result > (widen)(maxv)) {                \
            ls_panic_overflow(file, line, col, "subtract");                    \
        }                                                                      \
        return (ctype)result;                                                  \
    }                                                                          \
    ctype ls_mul_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        widen result = (widen)a * (widen)b;                                    \
        if (result < (widen)(minv) || result > (widen)(maxv)) {                \
            ls_panic_overflow(file, line, col, "multiply");                    \
        }                                                                      \
        return (ctype)result;                                                  \
    }                                                                          \
    ctype ls_div_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        widen result;                                                          \
        if (b == 0) {                                                          \
            ls_panic_div_zero(file, line, col);                                \
        }                                                                      \
        if ((widen)a == (widen)(minv) && (widen)b == (widen)-1) {              \
            ls_panic_overflow(file, line, col, "divide");                      \
        }                                                                      \
        result = (widen)a / (widen)b;                                          \
        return (ctype)result;                                                  \
    }                                                                          \
    ctype ls_rem_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        widen result;                                                          \
        if (b == 0) {                                                          \
            ls_rt_panic_rem_zero(file, line, col);                             \
        }                                                                      \
        if ((widen)a == (widen)(minv) && (widen)b == (widen)-1) {              \
            ls_rt_panic_rem_overflow(file, line, col);                         \
        }                                                                      \
        result = (widen)a % (widen)b;                                          \
        return (ctype)result;                                                  \
    }                                                                          \
    ctype ls_shl_##sfx##_checked(ctype a, int64_t b, const char* file,         \
                                 int32_t line, int32_t col) {                  \
        if (b < 0 || b >= LS_RT_BITS(ctype)) {                                 \
            ls_panic_overflow(file, line, col, "shift left");                  \
        }                                                                      \
        return (ctype)((utype)a << (unsigned)b);                               \
    }                                                                          \
    ctype ls_shr_##sfx##_checked(ctype a, int64_t b, const char* file,         \
                                 int32_t line, int32_t col) {                  \
        if (b < 0 || b >= LS_RT_BITS(ctype)) {                                 \
            ls_panic_overflow(file, line, col, "shift right");                 \
        }                                                                      \
        return (ctype)(a >> (unsigned)b);                                      \
    }                                                                          \
    ctype ls_neg_##sfx##_checked(ctype a, const char* file, int32_t line,      \
                                 int32_t col) {                                \
        if ((widen)a == (widen)(minv)) {                                       \
            ls_panic_overflow(file, line, col, "negate");                      \
        }                                                                      \
        return (ctype)(-a);                                                    \
    }

LS_RT_DEF_CHECKED_SIGNED_SMALL(i8, int8_t, int32_t, uint8_t, INT8_MIN, INT8_MAX)
LS_RT_DEF_CHECKED_SIGNED_SMALL(i16, int16_t, int32_t, uint16_t, INT16_MIN,
                               INT16_MAX)
LS_RT_DEF_CHECKED_SIGNED_SMALL(i32, int32_t, int64_t, uint32_t, INT32_MIN,
                               INT32_MAX)

#undef LS_RT_DEF_CHECKED_SIGNED_SMALL

/* ---------- smaller unsigned types: widening arithmetic ---------- */

#define LS_RT_DEF_CHECKED_UNSIGNED_SMALL(sfx, ctype, widen, maxv)              \
    ctype ls_add_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        widen result = (widen)a + (widen)b;                                    \
        if (result > (widen)(maxv)) {                                          \
            ls_panic_overflow(file, line, col, "add");                         \
        }                                                                      \
        return (ctype)result;                                                  \
    }                                                                          \
    ctype ls_sub_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        if (a < b) {                                                           \
            ls_panic_overflow(file, line, col, "subtract");                    \
        }                                                                      \
        return (ctype)(a - b);                                                 \
    }                                                                          \
    ctype ls_mul_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        widen result = (widen)a * (widen)b;                                    \
        if (result > (widen)(maxv)) {                                          \
            ls_panic_overflow(file, line, col, "multiply");                    \
        }                                                                      \
        return (ctype)result;                                                  \
    }                                                                          \
    ctype ls_div_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        if (b == 0) {                                                          \
            ls_panic_div_zero(file, line, col);                                \
        }                                                                      \
        return (ctype)(a / b);                                                 \
    }                                                                          \
    ctype ls_rem_##sfx##_checked(ctype a, ctype b, const char* file,           \
                                 int32_t line, int32_t col) {                  \
        if (b == 0) {                                                          \
            ls_rt_panic_rem_zero(file, line, col);                             \
        }                                                                      \
        return (ctype)(a % b);                                                 \
    }                                                                          \
    ctype ls_shl_##sfx##_checked(ctype a, int64_t b, const char* file,         \
                                 int32_t line, int32_t col) {                  \
        if (b < 0 || b >= LS_RT_BITS(ctype)) {                                 \
            ls_panic_overflow(file, line, col, "shift left");                  \
        }                                                                      \
        return (ctype)(a << (unsigned)b);                                      \
    }                                                                          \
    ctype ls_shr_##sfx##_checked(ctype a, int64_t b, const char* file,         \
                                 int32_t line, int32_t col) {                  \
        if (b < 0 || b >= LS_RT_BITS(ctype)) {                                 \
            ls_panic_overflow(file, line, col, "shift right");                 \
        }                                                                      \
        return (ctype)(a >> (unsigned)b);                                      \
    }

LS_RT_DEF_CHECKED_UNSIGNED_SMALL(u8, uint8_t, uint32_t, UINT8_MAX)
LS_RT_DEF_CHECKED_UNSIGNED_SMALL(u16, uint16_t, uint32_t, UINT16_MAX)
LS_RT_DEF_CHECKED_UNSIGNED_SMALL(u32, uint32_t, uint64_t, UINT32_MAX)

#undef LS_RT_DEF_CHECKED_UNSIGNED_SMALL
#undef LS_RT_BITS

/* ====================================================================== */
/* Process arguments and IO                                               */
/* ====================================================================== */

LsVec_String ls_args(void) {
    LsVec_String out = ls_vec_string_new();
    int i;

    if (ls_rt_argv == NULL) {
        return out;
    }
    /* argv[0] is the executable; the language exposes only user arguments. */
    for (i = 1; i < ls_rt_argc; i += 1) {
        if (ls_rt_argv[i] == NULL) {
            break;
        }
        ls_vec_string_push_owned(&out, ls_str_cstr(ls_rt_argv[i]));
    }
    return out;
}

LsOpt_String ls_read_line(void) {
    size_t capacity = 128;
    size_t length = 0;
    char* buffer = (char*)malloc(capacity);
    int c = EOF;
    LsString* line;

    if (buffer == NULL) {
        ls_rt_alloc_failure(capacity);
    }
    while ((c = fgetc(stdin)) != EOF) {
        if (length + 1 > capacity) {
            size_t new_capacity = capacity * 2;
            char* replacement = (char*)realloc(buffer, new_capacity);
            if (replacement == NULL) {
                free(buffer);
                ls_rt_alloc_failure(new_capacity);
            }
            buffer = replacement;
            capacity = new_capacity;
        }
        buffer[length] = (char)c;
        length += 1;
        if (c == '\n') {
            break;
        }
    }
    if (length == 0 && c == EOF) {
        free(buffer);
        return ls_opt_string_none();
    }
    if (length > 0 && buffer[length - 1] == '\n') {
        length -= 1;
    }
    if (length > 0 && buffer[length - 1] == '\r') {
        length -= 1;
    }
    line = ls_str_alloc_from(buffer, length);
    free(buffer);
    return ls_opt_string_some(line);
}

static bool ls_rt_path_has_nul(const LsString* path) {
    return path->len != 0 && memchr(path->data, '\0', path->len) != NULL;
}

static LsString* ls_rt_os_error_message(int error_number) {
    char buffer[256];
    const char* text;
    int written;

    text = strerror(error_number);
    if (text == NULL) {
        text = "unknown error";
    }
    written = snprintf(buffer, sizeof buffer, "%s (os error %d)", text,
                       error_number);
    if (written < 0) {
        written = 0;
    }
    if ((size_t)written > sizeof buffer - 1) {
        written = (int)(sizeof buffer - 1);
    }
    return ls_str_alloc_from(buffer, (size_t)written);
}

LsRes_String_String ls_read_file(const LsString* path) {
    FILE* file;
    char* buffer = NULL;
    size_t capacity = 0;
    size_t length = 0;
    int error_number = 0;

    if (ls_rt_path_has_nul(path)) {
        return ls_res_string_err(
            ls_str_cstr("file name contained an unexpected NUL byte"));
    }
    file = fopen(path->data, "rb");
    if (file == NULL) {
        return ls_res_string_err(ls_rt_os_error_message(errno));
    }
    for (;;) {
        size_t read_count;
        if (length == capacity) {
            size_t new_capacity = capacity == 0 ? 4096 : capacity * 2;
            char* replacement = (char*)realloc(buffer, new_capacity);
            if (replacement == NULL) {
                free(buffer);
                (void)fclose(file);
                ls_rt_alloc_failure(new_capacity);
            }
            buffer = replacement;
            capacity = new_capacity;
        }
        read_count = fread(buffer + length, 1, capacity - length, file);
        if (read_count == 0) {
            if (ferror(file) != 0) {
                error_number = errno;
            }
            break;
        }
        length += read_count;
    }
    if (fclose(file) != 0 && error_number == 0) {
        error_number = errno;
    }
    if (error_number != 0) {
        free(buffer);
        return ls_res_string_err(ls_rt_os_error_message(error_number));
    }
    {
        LsString* contents = ls_str_alloc_from(buffer, length);
        free(buffer);
        return ls_res_string_ok(contents);
    }
}

LsRes_String_String ls_write_file(const LsString* path,
                                  const LsString* contents) {
    FILE* file;
    int error_number = 0;

    if (ls_rt_path_has_nul(path)) {
        return ls_res_string_err(
            ls_str_cstr("file name contained an unexpected NUL byte"));
    }
    file = fopen(path->data, "wb");
    if (file == NULL) {
        return ls_res_string_err(ls_rt_os_error_message(errno));
    }
    if (contents->len != 0 &&
        fwrite(contents->data, 1, contents->len, file) != contents->len) {
        error_number = errno;
    }
    if (fclose(file) != 0 && error_number == 0) {
        error_number = errno;
    }
    if (error_number != 0) {
        return ls_res_string_err(ls_rt_os_error_message(error_number));
    }
    return ls_res_string_ok(ls_str_alloc_capacity(0));
}

/* ====================================================================== */
/* Math helpers                                                           */
/* ====================================================================== */

#define LS_RT_DEF_INT_MATH_SIGNED(sfx, ctype, minv)                            \
    ctype ls_abs_##sfx(ctype v) {                                              \
        if (v >= 0) {                                                          \
            return v;                                                          \
        }                                                                      \
        if (v == (ctype)(minv)) {                                              \
            return v; /* wrapping_abs: minimum stays minimum */                \
        }                                                                      \
        return (ctype)(-v);                                                    \
    }                                                                          \
    ctype ls_min_##sfx(ctype a, ctype b) { return a < b ? a : b; }             \
    ctype ls_max_##sfx(ctype a, ctype b) { return a > b ? a : b; }

#define LS_RT_DEF_INT_MATH_UNSIGNED(sfx, ctype)                                \
    ctype ls_abs_##sfx(ctype v) { return v; }                                  \
    ctype ls_min_##sfx(ctype a, ctype b) { return a < b ? a : b; }             \
    ctype ls_max_##sfx(ctype a, ctype b) { return a > b ? a : b; }

LS_RT_DEF_INT_MATH_SIGNED(i8, int8_t, INT8_MIN)
LS_RT_DEF_INT_MATH_SIGNED(i16, int16_t, INT16_MIN)
LS_RT_DEF_INT_MATH_SIGNED(i32, int32_t, INT32_MIN)
LS_RT_DEF_INT_MATH_SIGNED(i64, int64_t, INT64_MIN)
LS_RT_DEF_INT_MATH_UNSIGNED(u8, uint8_t)
LS_RT_DEF_INT_MATH_UNSIGNED(u16, uint16_t)
LS_RT_DEF_INT_MATH_UNSIGNED(u32, uint32_t)
LS_RT_DEF_INT_MATH_UNSIGNED(u64, uint64_t)

#undef LS_RT_DEF_INT_MATH_SIGNED
#undef LS_RT_DEF_INT_MATH_UNSIGNED

int8_t ls_abs_i8_checked(int8_t v, const char* file, int32_t line,
                         int32_t col) {
    if (v == INT8_MIN) {
        ls_panic_overflow(file, line, col, "negate");
    }
    return ls_abs_i8(v);
}

int16_t ls_abs_i16_checked(int16_t v, const char* file, int32_t line,
                           int32_t col) {
    if (v == INT16_MIN) {
        ls_panic_overflow(file, line, col, "negate");
    }
    return ls_abs_i16(v);
}

int32_t ls_abs_i32_checked(int32_t v, const char* file, int32_t line,
                           int32_t col) {
    if (v == INT32_MIN) {
        ls_panic_overflow(file, line, col, "negate");
    }
    return ls_abs_i32(v);
}

int64_t ls_abs_i64_checked(int64_t v, const char* file, int32_t line,
                           int32_t col) {
    if (v == INT64_MIN) {
        ls_panic_overflow(file, line, col, "negate");
    }
    return ls_abs_i64(v);
}

float ls_abs_f32(float v) {
    return fabsf(v);
}

double ls_abs_f64(double v) {
    return fabs(v);
}

float ls_min_f32(float a, float b) {
    return fminf(a, b);
}

double ls_min_f64(double a, double b) {
    return fmin(a, b);
}

float ls_max_f32(float a, float b) {
    return fmaxf(a, b);
}

double ls_max_f64(double a, double b) {
    return fmax(a, b);
}

float ls_sqrt_f32(float v) {
    return sqrtf(v);
}

double ls_sqrt_f64(double v) {
    return sqrt(v);
}

float ls_pow_f32(float base, float exponent) {
    return powf(base, exponent);
}

double ls_pow_f64(double base, double exponent) {
    return pow(base, exponent);
}

float ls_floor_f32(float v) {
    return floorf(v);
}

double ls_floor_f64(double v) {
    return floor(v);
}

float ls_ceil_f32(float v) {
    return ceilf(v);
}

double ls_ceil_f64(double v) {
    return ceil(v);
}

float ls_round_f32(float v) {
    return roundf(v);
}

double ls_round_f64(double v) {
    return round(v);
}

float ls_sin_f32(float v) {
    return sinf(v);
}

double ls_sin_f64(double v) {
    return sin(v);
}

float ls_cos_f32(float v) {
    return cosf(v);
}

double ls_cos_f64(double v) {
    return cos(v);
}

float ls_tan_f32(float v) {
    return tanf(v);
}

double ls_tan_f64(double v) {
    return tan(v);
}

float ls_ln_f32(float v) {
    return logf(v);
}

double ls_ln_f64(double v) {
    return log(v);
}

float ls_log10_f32(float v) {
    return log10f(v);
}

double ls_log10_f64(double v) {
    return log10(v);
}

float ls_exp_f32(float v) {
    return expf(v);
}

double ls_exp_f64(double v) {
    return exp(v);
}
