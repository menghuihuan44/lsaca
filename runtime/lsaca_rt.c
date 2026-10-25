/* lsaca C17 runtime: core translation unit.
 *
 * Reference counted allocations, the ORC trial-deletion cycle collector, the
 * runtime lifetime/leak report, panic reporting, checked integer arithmetic,
 * process/IO helpers and math helpers. The public ABI lives in lsaca_rt.h; the
 * private cross-unit contract lives in lsaca_rt_internal.h.
 *
 * Portable C17, no compiler extensions, no VLAs.
 */

#include "lsaca_rt_internal.h"

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

void ls_rt_configure_stdio(void) {
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

/* Cycle collector configuration (single threaded, see lsaca_rt.h). */
#define LS_RT_CANDIDATE_THRESHOLD ((size_t)512)
#define LS_RT_MAX_COLLECT_PASSES ((size_t)8)
#define LS_RT_COLOR_BLACK 0u
#define LS_RT_COLOR_GRAY 1u
#define LS_RT_COLOR_WHITE 2u
#define LS_RT_FLAG_QUEUED 0x01u       /* present in the candidate buffer */
#define LS_RT_FLAG_WHITE_LISTED 0x02u /* on the collector's white list */
/* All live reference counted allocations, most recently allocated first. */
static LsRtHeader* ls_rt_all_head = NULL;
static size_t ls_rt_live_count = 0;

#ifdef LS_RT_LEAK_TRACKING
/* Set once a shutdown has reported leaks; a second shutdown must stay silent. */
static int ls_rt_leak_reported = 0;
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

static void ls_rt_live_unlink(LsRtHeader* header);
static void ls_rt_candidate_push(LsRtHeader* header);
static void ls_rt_candidate_remove(LsRtHeader* header);
static void ls_rt_mark_gray(LsRtHeader* header);
static void ls_rt_scan(LsRtHeader* header);
static void ls_rt_scan_black(LsRtHeader* header);
static void ls_rt_collect_white(LsRtHeader* header);

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

/* Every allocation failure is fatal; there is no recoverable OOM in the
 * language. Print a diagnostic and exit with the panic exit code. */
LS_NORETURN void ls_rt_alloc_failure(size_t bytes) {
    ls_rt_configure_stdio();
    fflush(stdout);
    fprintf(stderr, "lsaca: memory allocation of %zu byte(s) failed\n", bytes);
    fflush(stderr);
    exit(101);
}

void* ls_rt_alloc_payload(size_t size, LsDropFn drop,
                                    LsTraceFn trace) {
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
    header->drop = drop;
    header->trace = trace;
    header->color = LS_RT_COLOR_BLACK;
    header->flags = 0;
    header->candidate_index = 0;
    header->all_prev = NULL;
    header->all_next = ls_rt_all_head;
    if (ls_rt_all_head != NULL) {
        ls_rt_all_head->all_prev = header;
    }
    ls_rt_all_head = header;
    ls_rt_live_count += 1;
    payload = ls_rt_payload_of(header);
    memset(payload, 0, size);
    return payload;
}

void* ls_alloc(size_t size) {
    return ls_rt_alloc_payload(size, NULL, NULL);
}

void* ls_alloc_managed(size_t size, LsDropFn drop, LsTraceFn trace) {
    return ls_rt_alloc_payload(size, drop, trace);
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

/* Diagnose an invalid or already released handle. This check is not a debug
 * feature: it catches use-after-free in every build configuration. */
LS_NORETURN static void ls_rt_release_invalid(void) {
    ls_rt_configure_stdio();
    fflush(stdout);
    fprintf(stderr,
            "lsaca: runtime error: release of an invalid or already "
            "released reference-counted object\n");
    fflush(stderr);
    exit(101);
}

void ls_release(void* obj) {
    LsRtHeader* header;

    if (obj == NULL) {
        return;
    }
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
        if ((header->flags & LS_RT_FLAG_QUEUED) == 0) {
            ls_rt_candidate_push(header);
        }
        /* Trigger a collection once enough candidates have accumulated. Never
         * collect from inside a collection or a drop function: the next
         * release or shutdown does it instead. */
        if (!ls_rt_collecting && ls_rt_free_depth == 0 &&
            (ls_rt_collect_pending ||
             ls_rt_candidate_count > ls_rt_candidate_threshold)) {
            ls_rt_collect_pending = 0;
            ls_collect_cycles();
        }
        return;
    }
    /* Final reference: free the payload normally. */
    header->refcount = 0;
    if ((header->flags & LS_RT_FLAG_QUEUED) != 0) {
        ls_rt_candidate_remove(header);
    }
    if (header->drop != NULL) {
        /* The payload is still intact, so its drop function can release the
         * handles it owns. free_depth suppresses a nested collection. */
        ls_rt_free_depth += 1;
        header->drop(ls_rt_payload_of(header));
        ls_rt_free_depth -= 1;
    }
    ls_rt_live_unlink(header);
    ls_rt_live_count -= 1;
    ls_rt_poison_header(header);
    free(header);
    if (!ls_rt_collecting && ls_rt_free_depth == 0 &&
        (ls_rt_collect_pending ||
         ls_rt_candidate_count > ls_rt_candidate_threshold)) {
        ls_rt_collect_pending = 0;
        ls_collect_cycles();
    }
}

size_t ls_refcount(const void* obj) {
    if (obj == NULL) {
        return 0;
    }
    return ls_rt_header_of(obj)->refcount;
}


/* ====================================================================== */
/* Cycle collector (trial deletion)                                       */
/* ====================================================================== */

/* Grow or allocate a pointer array; every runtime allocation failure is
 * fatal. */
static void* ls_rt_realloc_array(void* data, size_t count,
                                 size_t item_size) {
    void* next;
    if (count > SIZE_MAX / item_size) {
        ls_rt_alloc_failure(count);
    }
    next = realloc(data, count * item_size);
    if (next == NULL) {
        ls_rt_alloc_failure(count * item_size);
    }
    return next;
}

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
    header->flags |= LS_RT_FLAG_QUEUED;
}

static void ls_rt_candidate_remove(LsRtHeader* header) {
    size_t index = header->candidate_index;
    LsRtHeader* moved;
    if (index >= ls_rt_candidate_count ||
        ls_rt_candidates[index] != header) {
        /* Defensive: reaching this point would indicate corrupted bookkeeping;
         * keep going without corrupting the buffer further. */
        header->flags &= ~LS_RT_FLAG_QUEUED;
        return;
    }
    ls_rt_candidate_count -= 1;
    moved = ls_rt_candidates[ls_rt_candidate_count];
    if (index != ls_rt_candidate_count) {
        ls_rt_candidates[index] = moved;
        moved->candidate_index = index;
    }
    header->flags &= ~LS_RT_FLAG_QUEUED;
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
 * visit_mark is called once per edge; the decrement happens even when the
 * child was already gray, so each edge is subtracted exactly once. */
static void ls_rt_visit_mark(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        /* Unmanaged or untraceable object: its edges are never subtracted. */
        return;
    }
    header->refcount -= 1;
    ls_rt_mark_gray(header);
}

static void ls_rt_mark_gray(LsRtHeader* header) {
    if (header->color == LS_RT_COLOR_GRAY) {
        return;
    }
    header->color = LS_RT_COLOR_GRAY;
    if (header->trace != NULL) {
        header->trace(ls_rt_payload_of(header), ls_rt_visit_mark);
    }
}

/* Trial deletion phase 2: a gray object whose remaining (external) reference
 * count is positive is reachable from outside the trial set; paint it black
 * again and restore the counts of everything it reaches. Otherwise it is
 * garbage. */
static void ls_rt_visit_scan(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        return;
    }
    ls_rt_scan(header);
}

static void ls_rt_scan(LsRtHeader* header) {
    if (header->color != LS_RT_COLOR_GRAY) {
        return;
    }
    if (header->refcount > 0) {
        ls_rt_scan_black(header);
    } else {
        header->color = LS_RT_COLOR_WHITE;
    }
    if (header->trace != NULL) {
        header->trace(ls_rt_payload_of(header), ls_rt_visit_scan);
    }
}

static void ls_rt_visit_scan_black(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        return;
    }
    header->refcount += 1;
    ls_rt_scan_black(header);
}

static void ls_rt_scan_black(LsRtHeader* header) {
    if (header->color == LS_RT_COLOR_BLACK) {
        return;
    }
    header->color = LS_RT_COLOR_BLACK;
    if (header->trace != NULL) {
        header->trace(ls_rt_payload_of(header), ls_rt_visit_scan_black);
    }
}

/* Trial deletion phase 3: collect every white object into one list. */
static void ls_rt_visit_collect_white(void* child) {
    LsRtHeader* header = ls_rt_header_of(child);
    if (header->trace == NULL) {
        return;
    }
    ls_rt_collect_white(header);
}

static void ls_rt_collect_white(LsRtHeader* header) {
    if (header->color != LS_RT_COLOR_WHITE ||
        (header->flags & LS_RT_FLAG_WHITE_LISTED) != 0) {
        return;
    }
    header->flags |= LS_RT_FLAG_WHITE_LISTED;
    if (header->trace != NULL) {
        header->trace(ls_rt_payload_of(header), ls_rt_visit_collect_white);
    }
    ls_rt_white_push(header);
}

/* Free the collected white set. All drop functions run before any allocation
 * is freed, so a drop function always sees intact payloads of the other white
 * objects. */
static void ls_rt_free_whites(void) {
    size_t i;
    ls_rt_free_depth += 1;
    for (i = 0; i < ls_rt_white_count; i += 1) {
        LsRtHeader* header = ls_rt_white_list[i];
        if (header->drop != NULL) {
            header->drop(ls_rt_payload_of(header));
        }
    }
    for (i = 0; i < ls_rt_white_count; i += 1) {
        LsRtHeader* header = ls_rt_white_list[i];
        ls_rt_live_unlink(header);
        ls_rt_live_count -= 1;
        ls_rt_poison_header(header);
        header->flags = 0;
        free(header);
    }
    ls_rt_white_count = 0;
    ls_rt_free_depth -= 1;
}

void ls_collect_cycles(void) {
    size_t root_count;
    size_t i;
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
    for (i = 0; i < root_count; i += 1) {
        roots[i]->flags &= ~LS_RT_FLAG_QUEUED;
        roots[i]->candidate_index = 0;
    }
    ls_rt_collecting = 1;
    for (i = 0; i < root_count; i += 1) {
        ls_rt_mark_gray(roots[i]);
    }
    for (i = 0; i < root_count; i += 1) {
        ls_rt_scan(roots[i]);
    }
    ls_rt_white_count = 0;
    for (i = 0; i < root_count; i += 1) {
        ls_rt_collect_white(roots[i]);
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
#ifdef LS_RT_LEAK_TRACKING
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
#endif
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

