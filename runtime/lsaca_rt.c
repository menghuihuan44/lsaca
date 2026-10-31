/* lsaca C17 runtime: core translation unit.
 *
 * Reference counted allocations, the ORC trial-deletion cycle collector, the
 * runtime lifetime/leak report, panic reporting, checked integer arithmetic,
 * process/IO helpers and math helpers. The public ABI lives in lsaca_rt.h; the
 * private cross-unit contract lives in lsaca_rt_internal.h.
 *
 * Both graph traversals of the collector and the final-reference destruction
 * cascade are iterative: they use heap worklists instead of the C stack, so a
 * 100k-node chain or ring is collected/destroyed with O(1) call depth. Debug
 * builds additionally maintain the intrusive live-object list, the per-object
 * sizes, a live-handle hash set (stale-release diagnostics) and the shutdown
 * leak report; release builds compile all of that out.
 *
 * Portable C17, no compiler extensions, no VLAs.
 */

#include "lsaca_rt_internal.h"

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#else
#include <unistd.h>
#endif

/* Rust programs write stdout/stderr as raw byte streams. The Windows C runtime
 * would translate '\n' into "\r\n" when a stream is in text mode, which would
 * make print/println output, panic reports and line reads platform dependent.
 * Switch all three standard streams to binary mode once: ls_read_line() does
 * its own CRLF normalization and must preserve embedded NUL bytes and lone CR.
 *
 * stdout is line buffered (Rust's Stdout is a line writer: complete lines are
 * flushed immediately, output without a newline stays buffered) and stderr is
 * unbuffered so diagnostics are never lost. The print helpers in
 * lsaca_rt_fmt.c do not flush per call; ls_print_flush(), the panic reporter,
 * ls_exit() and ls_runtime_shutdown() are the explicit flush points. */
static int ls_rt_stdio_configured = 0;

void ls_rt_configure_stdio(void) {
    if (ls_rt_stdio_configured) {
        return;
    }
    ls_rt_stdio_configured = 1;
#ifdef _WIN32
    (void)_setmode(_fileno(stdin), _O_BINARY);
    (void)_setmode(_fileno(stdout), _O_BINARY);
    (void)_setmode(_fileno(stderr), _O_BINARY);
#endif
    (void)setvbuf(stdout, NULL, _IOLBF, 0);
    (void)setvbuf(stderr, NULL, _IONBF, 0);
}

/* ====================================================================== */
/* Reference counted allocations                                          */
/* ====================================================================== */

/* Cycle collector configuration (single threaded, see lsaca_rt.h). */
#define LS_RT_CANDIDATE_THRESHOLD ((size_t)512)
#define LS_RT_MAX_COLLECT_PASSES ((size_t)8)

/* Collector colors and object flags are packed into one byte in
 * LsRtHeader.state so that the release-mode header stays small. */
#define LS_RT_COLOR_MASK 0x03u
#define LS_RT_COLOR_BLACK 0x00u
#define LS_RT_COLOR_GRAY 0x01u
#define LS_RT_COLOR_WHITE 0x02u
/* LS_RT_FLAG_QUEUED / WHITE_LISTED / IMMORTAL are part of the private header
 * contract (see lsaca_rt_internal.h), because LsRtHeader.state is shared with
 * the string unit that allocates interned literals. */

/* Always maintained: the ls_rt_test_live_allocations() hook reports the same
 * numbers in every build configuration (one increment and one decrement per
 * allocation/free; the intrusive list itself is debug only). */
static size_t ls_rt_live_count = 0;

#if defined(LS_RT_LEAK_TRACKING)
/* All live reference counted allocations, most recently allocated first. The
 * intrusive list is maintained only in debug builds for the leak report. */
static LsRtHeader* ls_rt_all_head = NULL;

/* Set once a shutdown has reported leaks; a second shutdown must stay silent. */
static int ls_rt_leak_reported = 0;

/* Debug-only live-handle registry: an open-addressing hash set keyed by payload
 * pointer. It lets ls_release() reject stale (already released) handles even
 * when the allocator already reused the memory for a different live object,
 * instead of corrupting that object. Linear probing, power-of-two capacity,
 * tombstone deletion. */
#define LS_RT_REG_TOMBSTONE ((void*)(uintptr_t)1)
static void** ls_rt_registry = NULL;
static size_t ls_rt_registry_capacity = 0;
static size_t ls_rt_registry_count = 0;
static size_t ls_rt_registry_tombstones = 0;
#endif

/* Objects whose managed reference count dropped but stayed above zero. The
 * trial-deletion collector uses them as roots. */
static LsRtHeader** ls_rt_candidates = NULL;
static size_t ls_rt_candidate_count = 0;
static size_t ls_rt_candidate_capacity = 0;
static size_t ls_rt_candidate_threshold = LS_RT_CANDIDATE_THRESHOLD;

/* White objects found by the running collection; freed in one bulk at the end
 * so that all payloads are still intact while their drop functions run. */
static LsRtHeader** ls_rt_white_list = NULL;
static size_t ls_rt_white_count = 0;
static size_t ls_rt_white_capacity = 0;

/* Collector reentrancy guards: the language is single threaded, no atomics. */
static int ls_rt_collecting = 0;
static int ls_rt_free_depth = 0;
static int ls_rt_collect_pending = 0;

/* Iterative worklists: collector graph walks and the drop cascade never use the
 * C stack, so graph depth is bounded only by heap memory. */
typedef struct LsRtStack {
    LsRtHeader** items;
    size_t count;
    size_t capacity;
} LsRtStack;

static LsRtStack ls_rt_work_stack = {NULL, 0, 0};  /* mark/scan/white walk */
static LsRtStack ls_rt_black_stack = {NULL, 0, 0}; /* scan-black restore walk */
static LsRtStack ls_rt_free_stack = {NULL, 0, 0};  /* final-reference droppers */

static void ls_rt_live_unlink(LsRtHeader* header);
static void ls_rt_candidate_push(LsRtHeader* header);
static void ls_rt_candidate_remove(LsRtHeader* header);
static void ls_rt_poison_header(LsRtHeader* header);
static void ls_rt_free_header(LsRtHeader* header);
static void ls_rt_destroy(LsRtHeader* header);
static void ls_rt_drain_free_stack(void);
static void ls_rt_maybe_collect(void);
static void ls_rt_free_whites(void);
static void ls_rt_mark_gray(LsRtHeader* root);
static void ls_rt_scan(LsRtHeader* root);
static void ls_rt_scan_black(LsRtHeader* root);
static void ls_rt_collect_white(LsRtHeader* root);

static unsigned ls_rt_color(const LsRtHeader* header) {
    return (unsigned)(header->state & LS_RT_COLOR_MASK);
}

static void ls_rt_set_color(LsRtHeader* header, unsigned color) {
    header->state = (unsigned char)((header->state & (unsigned char)~LS_RT_COLOR_MASK) |
                                    (unsigned char)(color & LS_RT_COLOR_MASK));
}

/* Every allocation failure is fatal; there is no recoverable OOM in the
 * language. Print a diagnostic and exit with the panic exit code. */
LS_NORETURN void ls_rt_alloc_failure(size_t bytes) {
    ls_rt_configure_stdio();
    fflush(stdout);
    fprintf(stderr, "lsaca: memory allocation of %zu byte(s) failed\n", bytes);
    fflush(stderr);
    exit(101);
}

/* Grow or allocate an array; every runtime allocation failure is fatal. */
static void* ls_rt_realloc_array(void* data, size_t count, size_t item_size) {
    void* next;
    if (item_size != 0 && count > SIZE_MAX / item_size) {
        ls_rt_alloc_failure(count);
    }
    next = realloc(data, count * item_size);
    if (next == NULL && count != 0) {
        ls_rt_alloc_failure(count * item_size);
    }
    return next;
}

#if defined(LS_RT_LEAK_TRACKING)
static void ls_rt_live_unlink(LsRtHeader* header) {
    if (header->all_prev != NULL) {
        header->all_prev->all_next = header->all_next;
    } else {
        ls_rt_all_head = header->all_next;
    }
    if (header->all_next != NULL) {
        header->all_next->all_prev = header->all_prev;
    }
    header->all_prev = NULL;
    header->all_next = NULL;
}

static size_t ls_rt_hash_pointer(const void* pointer) {
    uint64_t value = (uint64_t)(uintptr_t)pointer;
    value += (uint64_t)0x9E3779B97F4A7C15ULL;
    value = (value ^ (value >> 30)) * (uint64_t)0xBF58476D1CE4E5B9ULL;
    value = (value ^ (value >> 27)) * (uint64_t)0x94D049BB133111EBULL;
    value ^= value >> 31;
    return (size_t)value;
}

static void ls_rt_registry_rebuild(size_t new_capacity) {
    void** old_entries = ls_rt_registry;
    size_t old_capacity = ls_rt_registry_capacity;
    size_t index;
    void** next = (void**)calloc(new_capacity, sizeof(void*));

    if (next == NULL) {
        ls_rt_alloc_failure(new_capacity * sizeof(void*));
    }
    ls_rt_registry = next;
    ls_rt_registry_capacity = new_capacity;
    ls_rt_registry_count = 0;
    ls_rt_registry_tombstones = 0;
    for (index = 0; index < old_capacity; index += 1) {
        void* entry = old_entries[index];
        size_t slot;
        size_t mask;
        if (entry == NULL || entry == LS_RT_REG_TOMBSTONE) {
            continue;
        }
        mask = new_capacity - 1;
        slot = ls_rt_hash_pointer(entry) & mask;
        while (ls_rt_registry[slot] != NULL) {
            slot = (slot + 1) & mask;
        }
        ls_rt_registry[slot] = entry;
        ls_rt_registry_count += 1;
    }
    free(old_entries);
}

static void ls_rt_registry_insert(void* payload) {
    size_t mask;
    size_t slot;

    if (ls_rt_registry_capacity == 0) {
        ls_rt_registry_rebuild(256);
    } else if ((ls_rt_registry_count + ls_rt_registry_tombstones + 1) * 10 >=
               ls_rt_registry_capacity * 7) {
        /* Grow only when most entries are live; when the table is mostly
         * tombstones, rehash at the same capacity (keeps long churn loops from
         * growing memory without bound). */
        size_t new_capacity = ls_rt_registry_capacity;
        if ((ls_rt_registry_count + 1) * 10 >= ls_rt_registry_capacity * 5) {
            new_capacity *= 2;
        }
        ls_rt_registry_rebuild(new_capacity);
    }
    mask = ls_rt_registry_capacity - 1;
    slot = ls_rt_hash_pointer(payload) & mask;
    while (ls_rt_registry[slot] != NULL &&
           ls_rt_registry[slot] != LS_RT_REG_TOMBSTONE) {
        slot = (slot + 1) & mask;
    }
    if (ls_rt_registry[slot] == LS_RT_REG_TOMBSTONE) {
        ls_rt_registry_tombstones -= 1;
    }
    ls_rt_registry[slot] = payload;
    ls_rt_registry_count += 1;
}

static int ls_rt_registry_contains(const void* payload) {
    size_t mask;
    size_t slot;

    if (ls_rt_registry_capacity == 0) {
        return 0;
    }
    mask = ls_rt_registry_capacity - 1;
    slot = ls_rt_hash_pointer(payload) & mask;
    for (;;) {
        void* entry = ls_rt_registry[slot];
        if (entry == NULL) {
            return 0;
        }
        if (entry == payload) {
            return 1;
        }
        slot = (slot + 1) & mask;
    }
}

static void ls_rt_registry_remove(const void* payload) {
    size_t mask;
    size_t slot;

    if (ls_rt_registry_capacity == 0) {
        return;
    }
    mask = ls_rt_registry_capacity - 1;
    slot = ls_rt_hash_pointer(payload) & mask;
    for (;;) {
        void* entry = ls_rt_registry[slot];
        if (entry == NULL) {
            return;
        }
        if (entry == payload) {
            ls_rt_registry[slot] = LS_RT_REG_TOMBSTONE;
            ls_rt_registry_count -= 1;
            ls_rt_registry_tombstones += 1;
            return;
        }
        slot = (slot + 1) & mask;
    }
}
#else
static void ls_rt_live_unlink(LsRtHeader* header) {
    (void)header;
}
#endif

static void ls_rt_stack_push(LsRtStack* stack, LsRtHeader* header) {
    if (stack->count == stack->capacity) {
        size_t new_capacity = stack->capacity == 0 ? 256 : stack->capacity * 2;
        stack->items = (LsRtHeader**)ls_rt_realloc_array(
            stack->items, new_capacity, sizeof(LsRtHeader*));
        stack->capacity = new_capacity;
    }
    stack->items[stack->count] = header;
    stack->count += 1;
}

static LsRtHeader* ls_rt_stack_pop(LsRtStack* stack) {
    stack->count -= 1;
    return stack->items[stack->count];
}

void* ls_rt_alloc_payload(size_t size, LsDropFn drop, LsTraceFn trace) {
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
    header->magic = LS_RT_LIVE_MAGIC;
    header->drop = drop;
    header->trace = trace;
    header->candidate_index = 0;
    header->refcount = 1;
    header->state = (unsigned char)LS_RT_COLOR_BLACK;
#if defined(LS_RT_LEAK_TRACKING)
    header->size = size;
    header->all_prev = NULL;
    header->all_next = ls_rt_all_head;
    if (ls_rt_all_head != NULL) {
        ls_rt_all_head->all_prev = header;
    }
    ls_rt_all_head = header;
#endif
    ls_rt_live_count += 1;
    payload = ls_rt_payload_of(header);
#if defined(LS_RT_LEAK_TRACKING)
    ls_rt_registry_insert(payload);
#endif
    memset(payload, 0, size);
    return payload;
}

void* ls_alloc(size_t size) {
    return ls_rt_alloc_payload(size, NULL, NULL);
}

void* ls_alloc_managed(size_t size, LsDropFn drop, LsTraceFn trace) {
    return ls_rt_alloc_payload(size, drop, trace);
}

/* Immortal payload (interned string literals): never freed, never leak-counted.
 * It still carries the live magic word so every release path can validate the
 * handle. The debug live-handle registry accepts it as a live payload, but the
 * intrusive leak list and the live counter deliberately omit it. */
void* ls_rt_alloc_immortal(size_t size) {
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
    header->magic = LS_RT_LIVE_MAGIC;
    header->drop = NULL;
    header->trace = NULL;
    header->candidate_index = 0;
    header->refcount = 1;
    header->state = (unsigned char)(LS_RT_COLOR_BLACK | LS_RT_FLAG_IMMORTAL);
#if defined(LS_RT_LEAK_TRACKING)
    /* Not part of the leak list: initialize the fields for determinism and only
     * register the handle for stale-release diagnostics. */
    header->size = size;
    header->all_prev = NULL;
    header->all_next = NULL;
#endif
    payload = ls_rt_payload_of(header);
#if defined(LS_RT_LEAK_TRACKING)
    ls_rt_registry_insert(payload);
#endif
    memset(payload, 0, size);
    return payload;
}

void* ls_retain(void* obj) {
    if (obj != NULL) {
        LsRtHeader* header = ls_rt_header_of(obj);
        header->refcount += 1;
    }
    return obj;
}

/* Clear the live magic word right before a header is freed. The store is
 * volatile on purpose: a plain store to memory that is about to be released
 * counts as dead and optimising compilers legitimately delete it, which would
 * leave the live magic word in the freed block and make a later release of the
 * stale handle corrupt the heap instead of reporting the runtime error. The
 * zero magic word is the marker of an invalid or already released handle. */
static void ls_rt_poison_header(LsRtHeader* header) {
    ((volatile LsRtHeader*)header)->magic = 0;
}

#if defined(LS_RT_LEAK_TRACKING)
/* Defined next to ls_rt_free_header(), after the release paths that report
 * invalid handles and free quarantine memory. */
static void ls_rt_quarantine_release_all(void);
#endif

/* Diagnose an invalid or already released handle. In debug builds the
 * live-handle registry catches stale handles reliably (even when the memory was
 * already reused); release builds fall back to the magic word, which is best
 * effort by nature. */
LS_NORETURN static void ls_rt_release_invalid(void) {
    ls_rt_configure_stdio();
#if defined(LS_RT_LEAK_TRACKING)
    ls_rt_quarantine_release_all();
#endif
    fflush(stdout);
    fprintf(stderr,
            "lsaca: runtime error: release of an invalid or already "
            "released reference-counted object\n");
    fflush(stderr);
    exit(101);
}

#if defined(LS_RT_LEAK_TRACKING)
/* Stale-handle quarantine.
 *
 * Debug builds do not hand freed payload blocks back to the allocator right
 * away. If they did, a fresh allocation could reuse the address of a released
 * handle and a later release of the stale handle would alias the new object:
 * the live-handle registry would accept the address and the release would
 * corrupt a live allocation. Keeping the block reserved (with a poisoned magic
 * word) until shutdown makes every recent stale handle detectable: the registry
 * no longer contains the address and the header memory is still readable. The
 * window is bounded so a long churn loop cannot grow without limit; blocks
 * evicted from the window (oldest first) fall back to the best-effort release
 * build behaviour. */
#define LS_RT_QUARANTINE_MAX ((size_t)262144)
static LsRtHeader* ls_rt_quarantine_head = NULL;
static LsRtHeader* ls_rt_quarantine_tail = NULL;
static size_t ls_rt_quarantine_count = 0;

/* Reuses the all_prev/all_next fields after ls_rt_live_unlink(): the block is
 * no longer part of the live list, so the links are free for the quarantine
 * list. */
static void ls_rt_quarantine_push(LsRtHeader* header) {
    header->all_prev = NULL;
    header->all_next = ls_rt_quarantine_head;
    if (ls_rt_quarantine_head != NULL) {
        ls_rt_quarantine_head->all_prev = header;
    }
    ls_rt_quarantine_head = header;
    if (ls_rt_quarantine_tail == NULL) {
        ls_rt_quarantine_tail = header;
    }
    ls_rt_quarantine_count += 1;
    if (ls_rt_quarantine_count > LS_RT_QUARANTINE_MAX) {
        LsRtHeader* oldest = ls_rt_quarantine_tail;
        ls_rt_quarantine_tail = oldest->all_prev;
        if (ls_rt_quarantine_tail != NULL) {
            ls_rt_quarantine_tail->all_next = NULL;
        } else {
            ls_rt_quarantine_head = NULL;
        }
        ls_rt_quarantine_count -= 1;
        free(oldest);
    }
}

/* Give the reserved blocks back once no user code can run any more. */
static void ls_rt_quarantine_release_all(void) {
    while (ls_rt_quarantine_head != NULL) {
        LsRtHeader* next = ls_rt_quarantine_head->all_next;
        free(ls_rt_quarantine_head);
        ls_rt_quarantine_head = next;
    }
    ls_rt_quarantine_tail = NULL;
    ls_rt_quarantine_count = 0;
}
#endif

static void ls_rt_free_header(LsRtHeader* header) {
    ls_rt_live_unlink(header);
    ls_rt_live_count -= 1;
#if defined(LS_RT_LEAK_TRACKING)
    ls_rt_registry_remove(ls_rt_payload_of(header));
#endif
    ls_rt_poison_header(header);
    header->state = 0;
#if defined(LS_RT_LEAK_TRACKING)
    /* Reserve the block for stale-handle diagnostics instead of freeing it. */
    ls_rt_quarantine_push(header);
#else
    free(header);
#endif
}

/* Run the pending drop cascade: one payload at a time, on the heap worklist. */
static void ls_rt_drain_free_stack(void) {
    while (ls_rt_free_stack.count != 0) {
        LsRtHeader* header = ls_rt_stack_pop(&ls_rt_free_stack);
        if (header->drop != NULL) {
            header->drop(ls_rt_payload_of(header));
        }
        ls_rt_free_header(header);
    }
}

/* Final reference released: enqueue the payload for destruction. Only the
 * outermost release context drains the queue, so drop functions that release
 * further final references extend the heap worklist instead of the C stack. */
static void ls_rt_destroy(LsRtHeader* header) {
    ls_rt_stack_push(&ls_rt_free_stack, header);
    if (ls_rt_free_depth > 0) {
        return;
    }
    ls_rt_free_depth += 1;
    ls_rt_drain_free_stack();
    ls_rt_free_depth -= 1;
    ls_rt_maybe_collect();
}

static void ls_rt_maybe_collect(void) {
    if (!ls_rt_collecting && ls_rt_free_depth == 0 &&
        (ls_rt_collect_pending ||
         ls_rt_candidate_count > ls_rt_candidate_threshold)) {
        ls_rt_collect_pending = 0;
        ls_collect_cycles();
    }
}

void ls_release(void* obj) {
    LsRtHeader* header;

    if (obj == NULL) {
        return;
    }
#if defined(LS_RT_LEAK_TRACKING)
    if (!ls_rt_registry_contains(obj)) {
        ls_rt_release_invalid();
    }
#endif
    header = ls_rt_header_of(obj);
    /* A released object has its magic cleared before free(), so a magic
     * mismatch detects both invalid and already released handles. */
    if (header->magic != LS_RT_LIVE_MAGIC) {
        ls_rt_release_invalid();
    }
    /* While the collector frees white garbage, every edge coming from the
     * white set was already subtracted by the trial deletion, whether the edge
     * target ended up white or stayed black (live). Running the white nodes'
     * drop functions must therefore not decrement those managed handles a
     * second time; the guarded nodes are exactly the ones the trace functions
     * visit (cycle nodes, trace != NULL). Unmanaged children (strings) are not
     * part of the trial graph and are released normally. */
    if (ls_rt_collecting && ls_rt_free_depth > 0 && header->trace != NULL) {
        return;
    }
    /* Immortal payloads (interned literals) are never destroyed: releasing one
     * only drops a counted reference, and the final "static" reference is left
     * in place. This works in release builds too, where the immortal flag is
     * the only marker available. */
    if ((header->state & LS_RT_FLAG_IMMORTAL) != 0) {
        if (header->refcount > 1) {
            header->refcount -= 1;
        }
        return;
    }
    if (header->refcount == 0) {
        ls_rt_release_invalid();
    }
    /* The collector only makes a candidate out of an object whose managed
     * reference count dropped but stayed above zero. */
    if (header->refcount > 1) {
        header->refcount -= 1;
        /* Untraced payloads (String, Vec data, plain structs) can never be
         * part of a cycle: their decrement ends here without touching any
         * collector global. This is the hot path of copy-on-write strings. */
        if (header->trace == NULL) {
            return;
        }
        if ((header->state & LS_RT_FLAG_QUEUED) == 0) {
            ls_rt_candidate_push(header);
        }
        ls_rt_maybe_collect();
        return;
    }
    /* Final reference: free the payload iteratively. */
    header->refcount = 0;
    if ((header->state & LS_RT_FLAG_QUEUED) != 0) {
        ls_rt_candidate_remove(header);
    }
    ls_rt_destroy(header);
}

size_t ls_refcount(const void* obj) {
    if (obj == NULL) {
        return 0;
    }
    return (size_t)ls_rt_header_of(obj)->refcount;
}

/* ====================================================================== */
/* Cycle collector (trial deletion)                                       */
/* ====================================================================== */

static void ls_rt_candidate_push(LsRtHeader* header) {
    if (ls_rt_candidate_count == ls_rt_candidate_capacity) {
        size_t new_capacity =
            ls_rt_candidate_capacity == 0 ? 64 : ls_rt_candidate_capacity * 2;
        ls_rt_candidates = (LsRtHeader**)ls_rt_realloc_array(
            ls_rt_candidates, new_capacity, sizeof(LsRtHeader*));
        ls_rt_candidate_capacity = new_capacity;
    }
    header->candidate_index = ls_rt_candidate_count;
    ls_rt_candidates[ls_rt_candidate_count] = header;
    ls_rt_candidate_count += 1;
    header->state |= (unsigned char)LS_RT_FLAG_QUEUED;
}

static void ls_rt_candidate_remove(LsRtHeader* header) {
    size_t index = header->candidate_index;
    LsRtHeader* moved;
    if (index >= ls_rt_candidate_count ||
        ls_rt_candidates[index] != header) {
        /* Defensive: reaching this point would indicate corrupted bookkeeping;
         * keep going without corrupting the buffer further. */
        header->state &= (unsigned char)~LS_RT_FLAG_QUEUED;
        return;
    }
    ls_rt_candidate_count -= 1;
    moved = ls_rt_candidates[ls_rt_candidate_count];
    if (index != ls_rt_candidate_count) {
        ls_rt_candidates[index] = moved;
        moved->candidate_index = index;
    }
    header->state &= (unsigned char)~LS_RT_FLAG_QUEUED;
}

static void ls_rt_white_push(LsRtHeader* header) {
    if (ls_rt_white_count == ls_rt_white_capacity) {
        size_t new_capacity =
            ls_rt_white_capacity == 0 ? 64 : ls_rt_white_capacity * 2;
        ls_rt_white_list = (LsRtHeader**)ls_rt_realloc_array(
            ls_rt_white_list, new_capacity, sizeof(LsRtHeader*));
        ls_rt_white_capacity = new_capacity;
    }
    ls_rt_white_list[ls_rt_white_count] = header;
    ls_rt_white_count += 1;
}

/* Trial deletion phase 1: gray every object reachable from the roots through
 * managed handles and subtract every internal edge from the reference counts.
 * The edge visitor is called once per edge; the decrement happens even when
 * the child was already gray, so each edge is subtracted exactly once. */
static void ls_rt_visit_mark(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        /* Unmanaged or untraceable object: its edges are never subtracted. */
        return;
    }
    header->refcount -= 1;
    if (ls_rt_color(header) != LS_RT_COLOR_GRAY) {
        ls_rt_set_color(header, LS_RT_COLOR_GRAY);
        ls_rt_stack_push(&ls_rt_work_stack, header);
    }
}

static void ls_rt_mark_gray(LsRtHeader* root) {
    if (ls_rt_color(root) == LS_RT_COLOR_GRAY) {
        return;
    }
    ls_rt_set_color(root, LS_RT_COLOR_GRAY);
    ls_rt_stack_push(&ls_rt_work_stack, root);
    while (ls_rt_work_stack.count != 0) {
        LsRtHeader* header = ls_rt_stack_pop(&ls_rt_work_stack);
        if (header->trace != NULL) {
            header->trace(ls_rt_payload_of(header), ls_rt_visit_mark);
        }
    }
}

/* Trial deletion phase 2: a gray object whose remaining (external) reference
 * count is positive is reachable from outside the trial set; paint it black
 * again and restore the counts of everything it reaches. Otherwise it is
 * garbage. Iterative version of the classic recursive scan. */
static void ls_rt_visit_scan_black(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        return;
    }
    header->refcount += 1;
    if (ls_rt_color(header) != LS_RT_COLOR_BLACK) {
        ls_rt_set_color(header, LS_RT_COLOR_BLACK);
        ls_rt_stack_push(&ls_rt_black_stack, header);
    }
}

static void ls_rt_scan_black(LsRtHeader* root) {
    if (ls_rt_color(root) == LS_RT_COLOR_BLACK) {
        return;
    }
    ls_rt_set_color(root, LS_RT_COLOR_BLACK);
    ls_rt_stack_push(&ls_rt_black_stack, root);
    while (ls_rt_black_stack.count != 0) {
        LsRtHeader* header = ls_rt_stack_pop(&ls_rt_black_stack);
        if (header->trace != NULL) {
            header->trace(ls_rt_payload_of(header), ls_rt_visit_scan_black);
        }
    }
}

static void ls_rt_visit_scan(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        return;
    }
    if (ls_rt_color(header) != LS_RT_COLOR_GRAY) {
        return;
    }
    ls_rt_stack_push(&ls_rt_work_stack, header);
}

static void ls_rt_scan(LsRtHeader* root) {
    if (ls_rt_color(root) != LS_RT_COLOR_GRAY) {
        return;
    }
    ls_rt_stack_push(&ls_rt_work_stack, root);
    while (ls_rt_work_stack.count != 0) {
        LsRtHeader* header = ls_rt_stack_pop(&ls_rt_work_stack);
        if (ls_rt_color(header) != LS_RT_COLOR_GRAY) {
            continue;
        }
        if (header->refcount > 0) {
            ls_rt_scan_black(header);
        } else {
            ls_rt_set_color(header, LS_RT_COLOR_WHITE);
        }
        if (header->trace != NULL) {
            header->trace(ls_rt_payload_of(header), ls_rt_visit_scan);
        }
    }
}

/* Trial deletion phase 3: collect every white object into one list. */
static void ls_rt_visit_collect_white(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        return;
    }
    if (ls_rt_color(header) == LS_RT_COLOR_WHITE &&
        (header->state & LS_RT_FLAG_WHITE_LISTED) == 0) {
        header->state |= (unsigned char)LS_RT_FLAG_WHITE_LISTED;
        ls_rt_stack_push(&ls_rt_work_stack, header);
    }
}

static void ls_rt_collect_white(LsRtHeader* root) {
    if (ls_rt_color(root) != LS_RT_COLOR_WHITE ||
        (root->state & LS_RT_FLAG_WHITE_LISTED) != 0) {
        return;
    }
    root->state |= (unsigned char)LS_RT_FLAG_WHITE_LISTED;
    ls_rt_stack_push(&ls_rt_work_stack, root);
    while (ls_rt_work_stack.count != 0) {
        LsRtHeader* header = ls_rt_stack_pop(&ls_rt_work_stack);
        if (header->trace != NULL) {
            header->trace(ls_rt_payload_of(header), ls_rt_visit_collect_white);
        }
        ls_rt_white_push(header);
    }
}

/* Free the collected white set. All drop functions run before any allocation
 * is freed, so a drop function always sees intact payloads of the other white
 * objects. Unmanaged children released by those drops are destroyed through the
 * heap free worklist. */
static void ls_rt_free_whites(void) {
    size_t index;
    ls_rt_free_depth += 1;
    for (index = 0; index < ls_rt_white_count; index += 1) {
        LsRtHeader* header = ls_rt_white_list[index];
        if (header->drop != NULL) {
            header->drop(ls_rt_payload_of(header));
        }
    }
    ls_rt_drain_free_stack();
    for (index = 0; index < ls_rt_white_count; index += 1) {
        ls_rt_free_header(ls_rt_white_list[index]);
    }
    ls_rt_white_count = 0;
    ls_rt_free_depth -= 1;
}

void ls_collect_cycles(void) {
    size_t root_count;
    size_t index;
    LsRtHeader** roots;

    if (ls_rt_collecting || ls_rt_free_depth > 0) {
        return;
    }
    if (ls_rt_candidate_count == 0) {
        ls_rt_collect_pending = 0;
        return;
    }
    root_count = ls_rt_candidate_count;
    roots = (LsRtHeader**)ls_rt_realloc_array(NULL, root_count,
                                              sizeof(LsRtHeader*));
    memcpy(roots, ls_rt_candidates, root_count * sizeof(LsRtHeader*));
    /* The running collection owns the roots; anything released from now on
     * (for example by a drop function while the whites are freed) is queued
     * again for the next collection. */
    ls_rt_candidate_count = 0;
    for (index = 0; index < root_count; index += 1) {
        roots[index]->state &= (unsigned char)~LS_RT_FLAG_QUEUED;
        roots[index]->candidate_index = 0;
    }
    ls_rt_collecting = 1;
    for (index = 0; index < root_count; index += 1) {
        ls_rt_mark_gray(roots[index]);
    }
    for (index = 0; index < root_count; index += 1) {
        ls_rt_scan(roots[index]);
    }
    ls_rt_white_count = 0;
    for (index = 0; index < root_count; index += 1) {
        ls_rt_collect_white(roots[index]);
    }
    if (ls_rt_white_count != 0) {
        ls_rt_free_whites();
    }
    ls_rt_collecting = 0;
    free(roots);
    ls_rt_collect_pending =
        ls_rt_candidate_count > ls_rt_candidate_threshold ? 1 : 0;
}

/* ---------------------------------------------------------------------- */
/* Test harness helpers. They are not part of the generated-code ABI; the */
/* smoke test declares them directly. Keeping them compiled into the       */
/* runtime lets the test run against an unmodified runtime build.          */
/* ---------------------------------------------------------------------- */

size_t ls_rt_test_live_allocations(void) {
    return ls_rt_live_count;
}

size_t ls_rt_test_candidate_threshold(void) {
    return ls_rt_candidate_threshold;
}

size_t ls_rt_test_set_candidate_threshold(size_t threshold) {
    size_t previous = ls_rt_candidate_threshold;
    ls_rt_candidate_threshold = threshold;
    return previous;
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
    size_t pass;

    ls_rt_configure_stdio();
    fflush(stdout);
    /* Collect garbage cycles before leak reporting: a program whose only
     * unreachable objects are cycles must not be reported as leaking. The
     * pass count is bounded so that a pathological drop function that keeps
     * producing candidates cannot loop forever. */
    ls_rt_collect_pending = 0;
    for (pass = 0; pass < LS_RT_MAX_COLLECT_PASSES; pass += 1) {
        if (ls_rt_candidate_count == 0) {
            break;
        }
        ls_collect_cycles();
    }
    ls_rt_collect_pending = 0;
#if defined(LS_RT_LEAK_TRACKING)
    if (!ls_rt_leak_reported) {
        size_t count = ls_rt_live_count;
        size_t shown = 0;
        LsRtHeader* header;
        if (count != 0) {
            fprintf(stderr,
                    "lsaca: warning: %zu reference-counted allocation(s) "
                    "leaked\n",
                    count);
            for (header = ls_rt_all_head;
                 header != NULL && shown < 8;
                 header = header->all_next, shown += 1) {
                fprintf(stderr, "lsaca:   leaked allocation of %zu bytes\n",
                        header->size);
            }
        }
        /* A second shutdown call must not report the same objects twice. */
        ls_rt_leak_reported = 1;
    }
    /* The debug stale-handle quarantine reserved released blocks; give that
     * memory back once user code cannot run any more. */
    ls_rt_quarantine_release_all();
#endif
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
#if defined(LS_RT_LEAK_TRACKING)
    ls_rt_quarantine_release_all();
#endif
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
#if defined(LS_RT_LEAK_TRACKING)
    ls_rt_quarantine_release_all();
#endif
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

/* Program arguments (excluding argv[0]) materialized as owned Strings. */
static LsVec_String ls_rt_args_located(const char* file, int32_t line,
                                       int32_t col) {
    LsVec_String out = ls_vec_string_new();
    int index;

    if (ls_rt_argv == NULL) {
        return out;
    }
    for (index = 1; index < ls_rt_argc; index += 1) {
        const char* argument = ls_rt_argv[index];
        size_t length;
        if (argument == NULL) {
            break;
        }
        length = strlen(argument);
        if (!ls_rt_utf8_validate(argument, length)) {
            ls_panic_cstr(file, line, col, "arguments contain invalid UTF-8");
        }
        ls_vec_string_push_owned(&out, ls_str_alloc_from(argument, length));
    }
    return out;
}

LsVec_String ls_args(void) {
    return ls_rt_args_located(NULL, 0, 0);
}

LsVec_String ls_args_at(const char* file, int32_t line, int32_t col) {
    return ls_rt_args_located(file, line, col);
}

/* Buffered stdin reader: 8 KiB fread() blocks scanned with memchr(). This keeps
 * long lines O(n) and stdin reads one syscall per block instead of one per
 * byte. The unconsumed tail of the block survives between calls. */
#define LS_RT_STDIN_BUFFER_SIZE ((size_t)8192)

static char ls_rt_stdin_buffer[LS_RT_STDIN_BUFFER_SIZE];
static size_t ls_rt_stdin_position = 0;
static size_t ls_rt_stdin_length = 0;
static int ls_rt_stdin_eof = 0;

/* Test-only hook: lets a regression program retarget stdin with freopen()
 * between cases without leaking buffered bytes from the previous file. */
void ls_rt_test_reset_stdin_buffer(void) {
    ls_rt_stdin_position = 0;
    ls_rt_stdin_length = 0;
    ls_rt_stdin_eof = 0;
}

static int ls_rt_stdin_fill(void) {
    size_t count;
    if (ls_rt_stdin_eof) {
        return 0;
    }
    count = fread(ls_rt_stdin_buffer, 1, LS_RT_STDIN_BUFFER_SIZE, stdin);
    if (count == 0) {
        ls_rt_stdin_eof = 1;
        return 0;
    }
    ls_rt_stdin_position = 0;
    ls_rt_stdin_length = count;
    return 1;
}

static void ls_rt_line_append(char** buffer, size_t* length, size_t* capacity,
                              const char* bytes, size_t count) {
    if (count > SIZE_MAX - *length) {
        ls_rt_alloc_failure(count);
    }
    if (*capacity - *length < count) {
        size_t new_capacity = *capacity == 0 ? 128 : *capacity;
        char* replacement;
        while (new_capacity - *length < count) {
            if (new_capacity > SIZE_MAX / 2) {
                new_capacity = *length + count;
                break;
            }
            new_capacity *= 2;
        }
        replacement = (char*)realloc(*buffer, new_capacity);
        if (replacement == NULL) {
            ls_rt_alloc_failure(new_capacity);
        }
        *buffer = replacement;
        *capacity = new_capacity;
    }
    memcpy(*buffer + *length, bytes, count);
    *length += count;
}

/* Read one line into a malloc'd buffer. The trailing newline is dropped; a CR
 * is dropped only when it is the CR of a CRLF pair (Rust read_line semantics:
 * a final line ending in a lone CR keeps that CR). Embedded NUL bytes are
 * preserved. Returns false on EOF with no bytes. */
static bool ls_rt_read_stdin_line(char** out_bytes, size_t* out_length) {
    char* buffer = NULL;
    size_t length = 0;
    size_t capacity = 0;
    bool complete = false;

    for (;;) {
        size_t available;
        const char* newline;
        size_t take;
        if (ls_rt_stdin_position == ls_rt_stdin_length) {
            if (!ls_rt_stdin_fill()) {
                break;
            }
        }
        available = ls_rt_stdin_length - ls_rt_stdin_position;
        newline = (const char*)memchr(
            ls_rt_stdin_buffer + ls_rt_stdin_position, '\n', available);
        take = newline != NULL
                   ? (size_t)(newline -
                              (ls_rt_stdin_buffer + ls_rt_stdin_position)) + 1
                   : available;
        ls_rt_line_append(&buffer, &length, &capacity,
                          ls_rt_stdin_buffer + ls_rt_stdin_position, take);
        ls_rt_stdin_position += take;
        if (newline != NULL) {
            complete = true;
            break;
        }
    }
    if (!complete && length == 0) {
        free(buffer);
        return false;
    }
    if (complete) {
        if (length > 0) {
            length -= 1; /* trailing '\n' */
        }
        if (length > 0 && buffer[length - 1] == '\r') {
            length -= 1; /* CR of a CRLF pair */
        }
    }
    *out_bytes = buffer;
    *out_length = length;
    return true;
}

static LsOpt_String ls_rt_read_line_located(const char* file, int32_t line,
                                            int32_t col) {
    char* bytes = NULL;
    size_t length = 0;
    const char* data;
    LsString* value;

    if (!ls_rt_read_stdin_line(&bytes, &length)) {
        return ls_opt_string_none();
    }
    data = bytes != NULL ? bytes : "";
    if (!ls_rt_utf8_validate(data, length)) {
        free(bytes);
        ls_panic_cstr(file, line, col, "stream did not contain valid UTF-8");
    }
    value = ls_str_alloc_from(data, length);
    free(bytes);
    return ls_opt_string_some(value);
}

LsOpt_String ls_read_line(void) {
    return ls_rt_read_line_located(NULL, 0, 0);
}

LsOpt_String ls_read_line_at(const char* file, int32_t line, int32_t col) {
    return ls_rt_read_line_located(file, line, col);
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
    if (!ls_rt_utf8_validate(length == 0 ? "" : buffer, length)) {
        free(buffer);
        return ls_res_string_err(
            ls_str_cstr("stream did not contain valid UTF-8"));
    }
    {
        LsString* contents = ls_str_alloc_from(length == 0 ? "" : buffer, length);
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

