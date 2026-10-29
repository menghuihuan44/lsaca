/* lsaca C17 runtime: internal shared contract between the translation units.
 *
 * PRIVATE header: it is not part of the generated-code ABI and generated code
 * never includes it. Only lsaca_rt.h is public. This header exists so the
 * runtime can be split into parallel translation units while sharing the
 * private allocation header and a handful of cross-unit helpers.
 *
 * The files and their concerns:
 *   lsaca_rt.c       allocations, ORC collector, lifetime/leak report, panics,
 *                    checked arithmetic, process/IO, math           (core)
 *   lsaca_rt_string.c  LsString objects, search/transform, split, parse
 *   lsaca_rt_vec.c     generic vector primitives, LsVec_String
 *   lsaca_rt_fmt.c     format builder, float formatting, output/printing
 *   lsaca_rt_map.c     hash map table, hashing/equality primitives
 */

#ifndef LSACA_RT_INTERNAL_H
#define LSACA_RT_INTERNAL_H

#include "lsaca_rt.h"

#include <errno.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Debug builds maintain the intrusive live-object list and report leaks that
 * survive ls_runtime_shutdown(). Release builds skip all of that bookkeeping. */
#if defined(LSACA_DEBUG) || defined(LSACA_DEBUG_RC)
#define LS_RT_LEAK_TRACKING 1
#endif

/* ====================================================================== */
/* Private allocation header (shared by core, String and Vec units)        */
/* ====================================================================== */

/* Magic stored in every live header. A zero magic word marks an invalid or
 * already released handle; ls_release() uses the mismatch to diagnose
 * use-after-free instead of corrupting the heap. */
#define LS_RT_LIVE_MAGIC ((uint64_t)0x4C53414341525431ULL)

/* Header stored immediately before every payload. The live-object list is
 * intrusive, so it needs no extra allocation; the payload follows the header
 * after padding to the strictest alignment. */
typedef struct LsRtHeader {
    size_t refcount;
    size_t size; /* payload size in bytes */
    uint64_t magic;
    LsDropFn drop;       /* NULL for unmanaged allocations */
    LsTraceFn trace;     /* NULL when the payload cannot hold managed children */
    unsigned char color; /* trial-deletion color, BLACK between collections */
    unsigned flags;      /* LS_RT_FLAG_* collector bookkeeping */
    struct LsRtHeader* all_prev; /* always maintained live-object list */
    struct LsRtHeader* all_next;
    size_t candidate_index; /* position in the candidate buffer while queued */
} LsRtHeader;

/* Distance from the malloc block to the payload, rounded up so the payload is
 * aligned for every C type. */
#define LS_RT_ALIGN ((size_t)_Alignof(max_align_t))
#define LS_RT_HEADER_SPACE \
    (((sizeof(LsRtHeader) + LS_RT_ALIGN - 1) / LS_RT_ALIGN) * LS_RT_ALIGN)

static inline LsRtHeader* ls_rt_header_of(const void* payload) {
    return (LsRtHeader*)(void*)((char*)payload - LS_RT_HEADER_SPACE);
}

static inline void* ls_rt_payload_of(LsRtHeader* header) {
    return (void*)((char*)header + LS_RT_HEADER_SPACE);
}

/* Reference count of a live payload; internal fast path for COW decisions. */
static inline size_t ls_rt_refcount_of(const void* payload) {
    return ls_rt_header_of(payload)->refcount;
}

/* ====================================================================== */
/* Helpers shared across translation units (defined exactly once)          */
/* ====================================================================== */

/* lsaca_rt.c (core). */
void ls_rt_configure_stdio(void);
LS_NORETURN void ls_rt_alloc_failure(size_t bytes);
void* ls_rt_alloc_payload(size_t size, LsDropFn drop, LsTraceFn trace);

/* lsaca_rt_string.c. */
LsString* ls_str_alloc_capacity(size_t capacity);
LsString* ls_str_alloc_from(const char* bytes, size_t len);
LsString* ls_str_append_bytes(LsString* s, const char* bytes, size_t count);
LsString* ls_str_append_slow(LsString* s, const char* bytes, size_t count);

/* In-place append fast path shared by the string and formatting units. When
 * the receiver is unique and already has room, the bytes and the terminating
 * NUL are recorded directly: no helper call, no reallocation and no temporary
 * string. Growing and copy-on-write detaching happen exactly once in the slow
 * path, which also preserves the caller's reference to the old object. */
static inline LsString* ls_str_append_fast(LsString* s, const char* bytes,
                                           size_t count) {
    if (s->cap - s->len >= count && ls_rt_refcount_of(s) == 1) {
        char* destination = s->data + s->len;
        if (count != 0 && bytes != NULL) {
            memcpy(destination, bytes, count);
        }
        s->len += count;
        s->data[s->len] = '\0';
        return s;
    }
    return ls_str_append_slow(s, bytes, count);
}

/* Append one byte through the same fast path. */
static inline LsString* ls_str_push_byte_fast(LsString* s, char c) {
    return ls_str_append_fast(s, &c, 1);
}

/* lsaca_rt_vec.c. */
void ls_vec_string_push_owned(LsVec_String* v, LsString* value);

#endif /* LSACA_RT_INTERNAL_H */
