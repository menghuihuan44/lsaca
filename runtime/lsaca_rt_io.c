/* lsaca C17 runtime: handle based operating-system I/O.
 *
 * This translation unit implements the file, directory, TCP, time and process
 * environment layer declared in lsaca_rt.h:
 *
 *   - a dynamically grown handle table shared by files and sockets, guarded by
 *     a small critical section;
 *   - file handles: open/create/read/write/seek/flush/size/close and
 *     nonblocking mode;
 *   - the per-thread last error: an error kind (see LsIoErrorKind) and an owned
 *     human readable message;
 *   - directories: read_dir, metadata, create/remove dir(s), remove_dir_all
 *     (iterative, no recursion depth limit), rename, canonicalize;
 *   - process state: current/temp directory, environment variables, arguments;
 *   - time: wall clock milliseconds, monotonic nanoseconds, sleep and local/UTC
 *     calendar breakdown;
 *   - TCP sockets: listen/accept/connect/local_port/timeout/shutdown.
 *
 * Windows: Win32 only (the C library is not used for paths or file handles),
 * with UTF-8 inputs converted to UTF-16. POSIX: plain POSIX.1-2008 syscalls.
 * Every entry point validates its arguments, resets the per-thread error to
 * LS_IO_ERROR_NONE and sets an error kind plus message on failure; returned
 * strings and vectors are fresh owned values (strings: LsString*, release with
 * ls_release(); vectors: LsVec_String*, release with ls_release() - their drop
 * function releases the elements and the buffer).
 *
 * Nothing here is part of the generated-code ABI except the declarations in
 * lsaca_rt.h. The generated code never includes this file. */

#if defined(_WIN32)
#  ifndef WIN32_LEAN_AND_MEAN
#    define WIN32_LEAN_AND_MEAN 1
#  endif
#  ifndef _WIN32_WINNT
#    define _WIN32_WINNT 0x0600
#  endif
#else
#  ifndef _POSIX_C_SOURCE
#    define _POSIX_C_SOURCE 200809L
#  endif
#  ifndef _DEFAULT_SOURCE
#    define _DEFAULT_SOURCE 1
#  endif
#  if defined(__APPLE__) && !defined(_DARWIN_C_SOURCE)
#    define _DARWIN_C_SOURCE 1
#  endif
#endif

#include "lsaca_rt_internal.h"

#include <stdarg.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#if defined(_WIN32)
#  include <winsock2.h>
#  include <ws2tcpip.h>
#  include <windows.h>
#  include <fcntl.h>
#  include <io.h>
#  include <sys/stat.h>
#  include <sys/types.h>
#else
#  include <dirent.h>
#  include <fcntl.h>
#  include <netdb.h>
#  include <netinet/in.h>
#  include <sys/socket.h>
#  include <sys/stat.h>
#  include <sys/time.h>
#  include <sys/types.h>
#  include <unistd.h>
#endif

/* ====================================================================== */
/* Per-thread last error                                                  */
/* ====================================================================== */

#define LS_RT_IO_INITIAL_ERROR_MESSAGE_MAX ((size_t)240)

static _Thread_local int64_t ls_rt_io_error_kind = LS_IO_ERROR_NONE;
static _Thread_local char
    ls_rt_io_error_message[LS_RT_IO_INITIAL_ERROR_MESSAGE_MAX + 1];

static void ls_rt_io_error_reset(void) {
    ls_rt_io_error_kind = LS_IO_ERROR_NONE;
    ls_rt_io_error_message[0] = '\0';
}

static int64_t ls_rt_io_fail(int64_t kind, const char* message) {
    ls_rt_io_error_kind = kind;
    if (message == NULL) {
        message = "";
    }
    (void)snprintf(ls_rt_io_error_message, sizeof ls_rt_io_error_message, "%s",
                   message);
    return -1;
}

/* POSIX and the Windows C library share the errno constants. */
static int64_t ls_rt_io_kind_from_errno(int error_number) {
    switch (error_number) {
        case ENOENT:
        case ENOTDIR:
#if defined(ENOFILE) && ENOFILE != ENOENT
        case ENOFILE:
#endif
            return LS_IO_ERROR_NOT_FOUND;
        case EACCES:
        case EPERM:
            return LS_IO_ERROR_PERMISSION_DENIED;
        case EEXIST:
            return LS_IO_ERROR_ALREADY_EXISTS;
        case EINVAL:
        case ENAMETOOLONG:
        case EBADF:
            return LS_IO_ERROR_INVALID_INPUT;
        case ECONNREFUSED:
            return LS_IO_ERROR_CONNECTION_REFUSED;
        case ECONNRESET:
            return LS_IO_ERROR_CONNECTION_RESET;
        case ECONNABORTED:
            return LS_IO_ERROR_CONNECTION_ABORTED;
        case ETIMEDOUT:
            return LS_IO_ERROR_TIMED_OUT;
        case EAGAIN:
#if defined(EWOULDBLOCK) && EWOULDBLOCK != EAGAIN
        case EWOULDBLOCK:
#endif
            return LS_IO_ERROR_WOULD_BLOCK;
        case EINTR:
            return LS_IO_ERROR_INTERRUPTED;
        case EPIPE:
#if defined(ESHUTDOWN) && ESHUTDOWN != EPIPE
        case ESHUTDOWN:
#endif
            return LS_IO_ERROR_BROKEN_PIPE;
        case EADDRINUSE:
            return LS_IO_ERROR_ADDRESS_IN_USE;
#if defined(EADDRNOTAVAIL)
        case EADDRNOTAVAIL:
#endif
#if defined(EAFNOSUPPORT)
        case EAFNOSUPPORT:
#endif
            return LS_IO_ERROR_ADDRESS_NOT_AVAILABLE;
#if defined(ENOTSUP) && (!defined(EOPNOTSUPP) || ENOTSUP != EOPNOTSUPP)
        case ENOTSUP:
#endif
#if defined(EOPNOTSUPP)
        case EOPNOTSUPP:
#endif
            return LS_IO_ERROR_UNSUPPORTED;
        default:
            return LS_IO_ERROR_OTHER;
    }
}

static int64_t ls_rt_io_fail_errno(int error_number) {
    char message[LS_RT_IO_INITIAL_ERROR_MESSAGE_MAX];
    const char* text = strerror(error_number);

    if (text == NULL) {
        text = "unknown error";
    }
    (void)snprintf(message, sizeof message, "%s (os error %d)", text,
                   error_number);
    return ls_rt_io_fail(ls_rt_io_kind_from_errno(error_number), message);
}

/* ====================================================================== */
/* Handle table shared by files and sockets                               */
/* ====================================================================== */

#define LS_RT_IO_KIND_FREE ((unsigned char)0)
#define LS_RT_IO_KIND_FILE ((unsigned char)1)
#define LS_RT_IO_KIND_SOCKET ((unsigned char)2)
#define LS_RT_IO_KIND_STD ((unsigned char)3)

#define LS_RT_IO_ALLOW_FILE ((unsigned int)(1u << LS_RT_IO_KIND_FILE))
#define LS_RT_IO_ALLOW_SOCKET ((unsigned int)(1u << LS_RT_IO_KIND_SOCKET))
#define LS_RT_IO_ALLOW_STD ((unsigned int)(1u << LS_RT_IO_KIND_STD))
#define LS_RT_IO_ALLOW_STREAM \
    (LS_RT_IO_ALLOW_FILE | LS_RT_IO_ALLOW_SOCKET | LS_RT_IO_ALLOW_STD)

#define LS_RT_IO_FLAG_NONBLOCKING ((unsigned char)0x01u)

/* Slots 0, 1 and 2 are the standard streams, exactly like file descriptors. */
#define LS_RT_IO_STDIN_HANDLE ((int64_t)0)
#define LS_RT_IO_STDOUT_HANDLE ((int64_t)1)
#define LS_RT_IO_STDERR_HANDLE ((int64_t)2)

#define LS_RT_IO_INITIAL_HANDLES ((size_t)64)
#define LS_RT_IO_NO_SLOT ((int32_t)-1)

typedef struct LsRtIoHandle {
    intptr_t native;      /* file descriptor, SOCKET or HANDLE */
    unsigned char kind;   /* LS_RT_IO_KIND_* */
    unsigned char flags;  /* LS_RT_IO_FLAG_* */
    int32_t next_free;    /* free list link while kind == FREE */
} LsRtIoHandle;

static LsRtIoHandle* ls_rt_io_handles = NULL;
static size_t ls_rt_io_handle_capacity = 0;
static int32_t ls_rt_io_free_head = LS_RT_IO_NO_SLOT;
static atomic_flag ls_rt_io_table_lock = ATOMIC_FLAG_INIT;
static int ls_rt_io_initialized = 0;

static void ls_rt_io_lock(void) {
    while (atomic_flag_test_and_set_explicit(&ls_rt_io_table_lock,
                                             memory_order_acquire)) {
        /* Spin: critical sections are a handful of instructions. */
    }
}

static void ls_rt_io_unlock(void) {
    atomic_flag_clear_explicit(&ls_rt_io_table_lock, memory_order_release);
}

/* Grow the table (called with the lock held, except during init). The new
 * slots form one free chain in ascending order so handles are reused
 * predictably: the lowest free slot is handed out first. */
static void ls_rt_io_table_grow(void) {
    size_t old_capacity = ls_rt_io_handle_capacity;
    size_t new_capacity = old_capacity == 0 ? LS_RT_IO_INITIAL_HANDLES
                                            : old_capacity * 2;
    LsRtIoHandle* grown =
        (LsRtIoHandle*)realloc(ls_rt_io_handles, new_capacity * sizeof *grown);
    size_t index;

    if (grown == NULL) {
        ls_rt_alloc_failure(new_capacity * sizeof *grown);
    }
    ls_rt_io_handles = grown;
    for (index = new_capacity; index > old_capacity; index -= 1) {
        LsRtIoHandle* slot = &ls_rt_io_handles[index - 1];
        slot->native = 0;
        slot->kind = LS_RT_IO_KIND_FREE;
        slot->flags = 0;
        slot->next_free = ls_rt_io_free_head;
        ls_rt_io_free_head = (int32_t)(index - 1);
    }
    ls_rt_io_handle_capacity = new_capacity;
}

/* Create the table and install the three standard stream handles. */
static void ls_rt_io_table_create(void) {
    size_t index;

    if (ls_rt_io_handles != NULL) {
        return;
    }
    ls_rt_io_table_grow();
    /* The freshly grown table chains every slot: drop 0, 1 and 2 from the free
     * list and turn them into the standard streams. */
    ls_rt_io_free_head = ls_rt_io_handles[2].next_free;
    for (index = 0; index < 3; index += 1) {
        LsRtIoHandle* slot = &ls_rt_io_handles[index];
        slot->native = (intptr_t)index;
        slot->kind = LS_RT_IO_KIND_STD;
        slot->flags = 0;
        slot->next_free = LS_RT_IO_NO_SLOT;
    }
}

static int64_t ls_rt_io_handle_allocate(unsigned char kind, intptr_t native) {
    int32_t slot;

    ls_rt_io_lock();
    if (ls_rt_io_free_head == LS_RT_IO_NO_SLOT) {
        ls_rt_io_table_grow();
    }
    slot = ls_rt_io_free_head;
    ls_rt_io_free_head = ls_rt_io_handles[slot].next_free;
    ls_rt_io_handles[slot].native = native;
    ls_rt_io_handles[slot].kind = kind;
    ls_rt_io_handles[slot].flags = 0;
    ls_rt_io_handles[slot].next_free = LS_RT_IO_NO_SLOT;
    ls_rt_io_unlock();
    return (int64_t)slot;
}

/* Copy the entry of a live handle into "out" when its kind is in "allowed".
 * Reports invalid_input for out of range, closed and wrong-kind handles. */
static bool ls_rt_io_handle_acquire(int64_t handle, unsigned int allowed,
                                    LsRtIoHandle* out) {
    bool found = false;

    ls_rt_io_lock();
    if (handle >= 0 && (uint64_t)handle < (uint64_t)ls_rt_io_handle_capacity) {
        LsRtIoHandle* slot = &ls_rt_io_handles[(size_t)handle];
        if (slot->kind != LS_RT_IO_KIND_FREE &&
            (allowed & (1u << slot->kind)) != 0) {
            *out = *slot;
            found = true;
        }
    }
    ls_rt_io_unlock();
    if (!found) {
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the io handle is invalid or was closed");
    }
    return found;
}

/* Remove a handle from the table. Standard streams cannot be closed. */
static bool ls_rt_io_handle_detach(int64_t handle, LsRtIoHandle* out) {
    bool found = false;

    ls_rt_io_lock();
    if (handle >= 0 && (uint64_t)handle < (uint64_t)ls_rt_io_handle_capacity) {
        LsRtIoHandle* slot = &ls_rt_io_handles[(size_t)handle];
        if (slot->kind != LS_RT_IO_KIND_FREE &&
            slot->kind != LS_RT_IO_KIND_STD) {
            *out = *slot;
            slot->kind = LS_RT_IO_KIND_FREE;
            slot->flags = 0;
            slot->native = 0;
            slot->next_free = ls_rt_io_free_head;
            ls_rt_io_free_head = (int32_t)handle;
            found = true;
        }
    }
    ls_rt_io_unlock();
    if (!found) {
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the io handle is invalid or was closed");
    }
    return found;
}


/* ====================================================================== */
/* Shared helpers: owned values, path text, network addresses             */
/* ====================================================================== */

/* Drop hook for owned vectors of strings returned to generated code. */
static void ls_rt_io_vec_string_drop(void* payload) {
    ls_vec_string_free((LsVec_String*)payload);
}

/* Allocate an owned vector whose destruction releases the elements and the
 * buffer. Returns NULL only on allocation failure (which is fatal). */
static LsVec_String* ls_rt_io_vec_string_new(void) {
    LsVec_String* vector = (LsVec_String*)ls_alloc_managed(
        sizeof *vector, ls_rt_io_vec_string_drop, NULL);

    *vector = ls_vec_string_new();
    return vector;
}

/* Build a fresh owned string from bytes that may not be valid UTF-8 (OS error
 * text, environment values on POSIX). Invalid bytes are replaced with '?' so
 * that the runtime invariant "every LsString is valid UTF-8" holds. */
static LsString* ls_rt_io_string_lossy(const char* bytes, size_t len) {
    LsString* result;
    char* sanitized;
    size_t index;

    if (len == 0) {
        return ls_str_alloc_from("", 0);
    }
    if (ls_rt_utf8_validate(bytes, len)) {
        return ls_str_alloc_from(bytes, len);
    }
    sanitized = (char*)malloc(len);
    if (sanitized == NULL) {
        ls_rt_alloc_failure(len);
    }
    for (index = 0; index < len; index += 1) {
        unsigned char byte = (unsigned char)bytes[index];
        sanitized[index] = byte < 0x80u ? (char)byte : '?';
    }
    result = ls_str_alloc_from(sanitized, len);
    free(sanitized);
    return result;
}

/* Copy an LsString used as a NUL terminated OS string (path, address, variable
 * name). Embedded NUL bytes are rejected because they would truncate the OS
 * string. Returns NULL on failure (the error is recorded). */
static char* ls_rt_io_text_copy(const LsString* text, const char* what) {
    char* copy;

    if (text == NULL) {
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the value is null");
        return NULL;
    }
    if (text->len != 0 && memchr(text->data, 0, text->len) != NULL) {
        char message[128];
        (void)snprintf(message, sizeof message, "the %s contains a NUL byte",
                       what);
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT, message);
        return NULL;
    }
    copy = (char*)malloc(text->len + 1);
    if (copy == NULL) {
        ls_rt_alloc_failure(text->len + 1);
    }
    if (text->len != 0) {
        memcpy(copy, text->data, text->len);
    }
    copy[text->len] = '\0';
    return copy;
}

/* Report that the given handle kind does not support an operation. */
static int64_t ls_rt_io_fail_unsupported(const char* operation) {
    char message[160];
    (void)snprintf(message, sizeof message,
                   "the io handle does not support %s", operation);
    (void)ls_rt_io_fail(LS_IO_ERROR_UNSUPPORTED, message);
    return -1;
}

/* Parse "host:port" into a NUL terminated host name and a decimal port string
 * (both malloc'ed, released by the caller). The host may be empty, a name, an
 * IPv4 address or an IPv6 address in square brackets; the port must be a
 * decimal number in 0..65535 (0 means "any port" for listeners). Returns 0 on
 * success and -1 on failure with the error recorded. */
static int ls_rt_io_parse_address(const LsString* address, char** out_host,
                                  char** out_port) {
    char* text;
    char* colon;
    char* host_text;
    char* port_text;
    size_t host_length;
    size_t port_length;
    size_t index;
    long port = 0;

    *out_host = NULL;
    *out_port = NULL;
    text = ls_rt_io_text_copy(address, "address");
    if (text == NULL) {
        return -1;
    }
    colon = strrchr(text, ':');
    if (colon == NULL) {
        free(text);
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the address must use the host:port form");
        return -1;
    }
    port_text = colon + 1;
    port_length = strlen(port_text);
    if (port_length == 0) {
        free(text);
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the address is missing a port number");
        return -1;
    }
    for (index = 0; index < port_length; index += 1) {
        if (port_text[index] < '0' || port_text[index] > '9') {
            free(text);
            (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                                "the port must be a decimal number");
            return -1;
        }
        port = port * 10 + (port_text[index] - '0');
        if (port > 65535) {
            free(text);
            (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                                "the port must be between 0 and 65535");
            return -1;
        }
    }
    host_text = text;
    host_length = (size_t)(colon - text);
    if (host_length >= 2 && host_text[0] == '[' &&
        host_text[host_length - 1] == ']') {
        host_text += 1;
        host_length -= 2;
    } else if (memchr(host_text, ':', host_length) != NULL) {
        free(text);
        (void)ls_rt_io_fail(
            LS_IO_ERROR_INVALID_INPUT,
            "IPv6 addresses must be written in square brackets, like [::1]:8080");
        return -1;
    }
    *out_host = (char*)malloc(host_length + 1);
    if (*out_host == NULL) {
        free(text);
        ls_rt_alloc_failure(host_length + 1);
    }
    if (host_length != 0) {
        memcpy(*out_host, host_text, host_length);
    }
    (*out_host)[host_length] = '\0';
    *out_port = (char*)malloc(port_length + 1);
    if (*out_port == NULL) {
        free(*out_host);
        *out_host = NULL;
        free(text);
        ls_rt_alloc_failure(port_length + 1);
    }
    memcpy(*out_port, port_text, port_length);
    (*out_port)[port_length] = '\0';
    free(text);
    return 0;
}

/* ====================================================================== */
/* Platform independent name lists (directory entries)                    */
/* ====================================================================== */

#if defined(_WIN32)
typedef wchar_t LsRtIoNameChar;
#else
typedef char LsRtIoNameChar;
#endif

typedef struct LsRtIoNameList {
    LsRtIoNameChar** items;
    size_t count;
    size_t capacity;
} LsRtIoNameList;

/* Append an owned name (the list takes ownership of the allocation). */
static void ls_rt_io_name_list_push(LsRtIoNameList* list,
                                    LsRtIoNameChar* name) {
    if (list->count == list->capacity) {
        size_t grown_capacity = list->capacity == 0 ? 8 : list->capacity * 2;
        LsRtIoNameChar** grown = (LsRtIoNameChar**)realloc(
            list->items, grown_capacity * sizeof *grown);
        if (grown == NULL) {
            ls_rt_alloc_failure(grown_capacity * sizeof *grown);
        }
        list->items = grown;
        list->capacity = grown_capacity;
    }
    list->items[list->count] = name;
    list->count += 1;
}

static int ls_rt_io_name_compare(const void* left, const void* right) {
#if defined(_WIN32)
    return wcscmp(*(LsRtIoNameChar* const*)left,
                  *(LsRtIoNameChar* const*)right);
#else
    return strcmp(*(LsRtIoNameChar* const*)left,
                  *(LsRtIoNameChar* const*)right);
#endif
}

static void ls_rt_io_name_list_sort(LsRtIoNameList* list) {
    if (list->count > 1) {
        qsort(list->items, list->count, sizeof *list->items,
              ls_rt_io_name_compare);
    }
}

static void ls_rt_io_name_list_free(LsRtIoNameList* list) {
    size_t index;

    for (index = 0; index < list->count; index += 1) {
        free(list->items[index]);
    }
    free(list->items);
    list->items = NULL;
    list->count = 0;
    list->capacity = 0;
}
/* ====================================================================== */
/* Shared helpers: strings, addresses, sorting                            */
/* ====================================================================== */

/* Compare two owned LsString values by their bytes (used to sort environment
 * variable "NAME=VALUE" entries and directory entries). */
static int ls_rt_io_string_compare(const void* left, const void* right) {
    const LsString* a = *(const LsString* const*)left;
    const LsString* b = *(const LsString* const*)right;
    size_t common = a->len < b->len ? a->len : b->len;
    int order = common == 0 ? 0 : memcmp(a->data, b->data, common);

    if (order != 0) {
        return order;
    }
    if (a->len == b->len) {
        return 0;
    }
    return a->len < b->len ? -1 : 1;
}

/* Resolve a "host:port" address into a getaddrinfo list for a TCP stream.
 * for_listen selects passive addresses (an empty host means every local
 * address). Returns 0 on success and -1 with the per-thread error set. */
static int ls_rt_io_resolve_address(const LsString* address, bool for_listen,
                                    struct addrinfo** out) {
    char* host = NULL;
    char* port = NULL;
    struct addrinfo hints;
    int status;

    *out = NULL;
    if (ls_rt_io_parse_address(address, &host, &port) != 0) {
        return -1;
    }
    memset(&hints, 0, sizeof hints);
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_protocol = IPPROTO_TCP;
    if (for_listen) {
        hints.ai_flags = AI_PASSIVE;
    }
    status = getaddrinfo(host[0] == '\0' ? NULL : host, port, &hints, out);
    free(host);
    free(port);
    if (status != 0) {
        const char* text = gai_strerror(status);
        char message[LS_RT_IO_INITIAL_ERROR_MESSAGE_MAX];

        (void)snprintf(message, sizeof message,
                       "the address could not be resolved: %s",
                       text == NULL ? "unknown error" : text);
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT, message);
        return -1;
    }
    return 0;
}

/* ====================================================================== */
/* Windows (Win32) implementation                                         */
/* ====================================================================== */

#if defined(_WIN32)

/* --- UTF-8 / UTF-16 conversion --- */

/* Convert UTF-8 bytes to a NUL terminated UTF-16 string. The caller owns the
 * result. Records invalid_input when the bytes are not valid UTF-8. */
static wchar_t* ls_rt_io_wide_from_bytes(const char* bytes, size_t len) {
    int required;
    wchar_t* wide;

    if (len > (size_t)INT_MAX) {
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT, "the text is too long");
        return NULL;
    }
    required = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, bytes,
                                   (int)len, NULL, 0);
    if (required <= 0 && len != 0) {
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the text is not valid UTF-8");
        return NULL;
    }
    wide = (wchar_t*)malloc(((size_t)required + 1) * sizeof *wide);
    if (wide == NULL) {
        ls_rt_alloc_failure(((size_t)required + 1) * sizeof *wide);
    }
    if (required > 0) {
        int written = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, bytes,
                                          (int)len, wide, required);
        if (written != required) {
            free(wide);
            (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                                "the text is not valid UTF-8");
            return NULL;
        }
    }
    wide[required] = L'\0';
    return wide;
}

/* Convert a NUL terminated UTF-16 string into an owned UTF-8 C string (plain
 * malloc, caller frees). Records invalid_input when the wide text cannot be
 * represented as UTF-8. */
static char* ls_rt_io_utf8_from_wide_alloc(const wchar_t* wide) {
    int length;
    int required;
    char* buffer;

    length = (int)wcslen(wide);
    required = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, length,
                                   NULL, 0, NULL, NULL);
    if (required <= 0 && length != 0) {
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the text is not valid UTF-16");
        return NULL;
    }
    buffer = (char*)malloc((size_t)required + 1);
    if (buffer == NULL) {
        ls_rt_alloc_failure((size_t)required + 1);
    }
    if (required > 0) {
        int written = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide,
                                          length, buffer, required, NULL, NULL);
        if (written != required) {
            free(buffer);
            (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                                "the text is not valid UTF-16");
            return NULL;
        }
    }
    buffer[required] = '\0';
    return buffer;
}

/* Convert a NUL terminated UTF-16 string into an owned LsString (valid UTF-8).
 * Records invalid_input when the conversion is impossible. */
static LsString* ls_rt_io_string_from_wide(const wchar_t* wide) {
    char* buffer = ls_rt_io_utf8_from_wide_alloc(wide);
    LsString* result;

    if (buffer == NULL) {
        return NULL;
    }
    result = ls_str_alloc_from(buffer, strlen(buffer));
    free(buffer);
    return result;
}

/* --- Process arguments ------------------------------------------------- */

/* Dynamic wide-string buffer used while splitting the command line. */
typedef struct LsRtIoWideBuffer {
    wchar_t* data;
    size_t length;
    size_t capacity;
} LsRtIoWideBuffer;

static bool ls_rt_io_wide_buffer_push(LsRtIoWideBuffer* buffer, wchar_t value) {
    wchar_t* data;
    size_t capacity;

    if (buffer->length == buffer->capacity) {
        capacity = buffer->capacity == 0 ? 16 : buffer->capacity * 2;
        if (capacity < buffer->capacity ||
            capacity > SIZE_MAX / sizeof *data) {
            return false;
        }
        data = (wchar_t*)realloc(buffer->data, capacity * sizeof *data);
        if (data == NULL) {
            return false;
        }
        buffer->data = data;
        buffer->capacity = capacity;
    }
    buffer->data[buffer->length] = value;
    buffer->length += 1;
    return true;
}

static void ls_rt_io_wide_buffer_dispose(LsRtIoWideBuffer* buffer) {
    free(buffer->data);
    buffer->data = NULL;
    buffer->length = 0;
    buffer->capacity = 0;
}

/* Dynamic list of owned wide arguments. */
typedef struct LsRtIoWideArguments {
    wchar_t** items;
    size_t count;
    size_t capacity;
} LsRtIoWideArguments;

static bool ls_rt_io_wide_arguments_push(LsRtIoWideArguments* list,
                                         wchar_t* item) {
    wchar_t** items;
    size_t capacity;

    if (list->count == list->capacity) {
        capacity = list->capacity == 0 ? 8 : list->capacity * 2;
        if (capacity < list->capacity || capacity > SIZE_MAX / sizeof *items) {
            return false;
        }
        items = (wchar_t**)realloc(list->items, capacity * sizeof *items);
        if (items == NULL) {
            return false;
        }
        list->items = items;
        list->capacity = capacity;
    }
    list->items[list->count] = item;
    list->count += 1;
    return true;
}

static void ls_rt_io_wide_arguments_dispose(LsRtIoWideArguments* list) {
    size_t index;

    for (index = 0; index < list->count; index += 1) {
        free(list->items[index]);
    }
    free(list->items);
    list->items = NULL;
    list->count = 0;
    list->capacity = 0;
}

static bool ls_rt_io_is_argument_space(wchar_t value) {
    return value == L' ' || value == L'\t';
}

/* Split a Windows command line into wide arguments with the documented
 * CommandLineToArgvW rules (the same rules the C runtime uses): blanks separate
 * arguments, double quotes group them, backslashes before a quote follow the
 * 2n / 2n+1 rule (2n backslashes plus a quote produce n backslashes and toggle
 * quoting, 2n+1 produce n backslashes and a literal quote), and argv[0] is
 * parsed with the simpler "leading quote to the next quote" rule. Returns 0 on
 * success and -1 when memory runs out. */
static int ls_rt_io_split_command_line(const wchar_t* command_line,
                                       LsRtIoWideArguments* out) {
    const wchar_t* cursor = command_line;
    bool first = true;

    while (true) {
        LsRtIoWideBuffer buffer;
        bool in_quotes = false;

        while (*cursor != L'\0' && ls_rt_io_is_argument_space(*cursor)) {
            cursor += 1;
        }
        if (*cursor == L'\0') {
            break;
        }
        buffer.data = NULL;
        buffer.length = 0;
        buffer.capacity = 0;
        if (first) {
            if (*cursor == L'"') {
                cursor += 1;
                while (*cursor != L'\0' && *cursor != L'"') {
                    if (!ls_rt_io_wide_buffer_push(&buffer, *cursor)) {
                        ls_rt_io_wide_buffer_dispose(&buffer);
                        return -1;
                    }
                    cursor += 1;
                }
                if (*cursor == L'"') {
                    cursor += 1;
                }
            } else {
                while (*cursor != L'\0' &&
                       !ls_rt_io_is_argument_space(*cursor)) {
                    if (!ls_rt_io_wide_buffer_push(&buffer, *cursor)) {
                        ls_rt_io_wide_buffer_dispose(&buffer);
                        return -1;
                    }
                    cursor += 1;
                }
            }
            first = false;
        } else {
            while (*cursor != L'\0' &&
                   (in_quotes || !ls_rt_io_is_argument_space(*cursor))) {
                if (*cursor == L'\\') {
                    size_t backslashes = 0;
                    size_t index;

                    while (*cursor == L'\\') {
                        backslashes += 1;
                        cursor += 1;
                    }
                    if (*cursor == L'"') {
                        for (index = 0; index < backslashes / 2; index += 1) {
                            if (!ls_rt_io_wide_buffer_push(&buffer, L'\\')) {
                                ls_rt_io_wide_buffer_dispose(&buffer);
                                return -1;
                            }
                        }
                        if ((backslashes % 2) != 0) {
                            if (!ls_rt_io_wide_buffer_push(&buffer, L'"')) {
                                ls_rt_io_wide_buffer_dispose(&buffer);
                                return -1;
                            }
                            cursor += 1;
                        } else {
                            in_quotes = !in_quotes;
                            cursor += 1;
                        }
                    } else {
                        for (index = 0; index < backslashes; index += 1) {
                            if (!ls_rt_io_wide_buffer_push(&buffer, L'\\')) {
                                ls_rt_io_wide_buffer_dispose(&buffer);
                                return -1;
                            }
                        }
                    }
                } else if (*cursor == L'"') {
                    if (in_quotes && cursor[1] == L'"') {
                        if (!ls_rt_io_wide_buffer_push(&buffer, L'"')) {
                            ls_rt_io_wide_buffer_dispose(&buffer);
                            return -1;
                        }
                        cursor += 2;
                    } else {
                        in_quotes = !in_quotes;
                        cursor += 1;
                    }
                } else {
                    if (!ls_rt_io_wide_buffer_push(&buffer, *cursor)) {
                        ls_rt_io_wide_buffer_dispose(&buffer);
                        return -1;
                    }
                    cursor += 1;
                }
            }
        }
        if (!ls_rt_io_wide_buffer_push(&buffer, L'\0')) {
            ls_rt_io_wide_buffer_dispose(&buffer);
            return -1;
        }
        if (!ls_rt_io_wide_arguments_push(out, buffer.data)) {
            ls_rt_io_wide_buffer_dispose(&buffer);
            return -1;
        }
    }
    return 0;
}

char** ls_rt_io_utf8_argv(int* out_argc) {
    LsRtIoWideArguments wide;
    const wchar_t* command_line;
    char** result;
    size_t index;
    int count;

    if (out_argc == NULL) {
        return NULL;
    }
    *out_argc = 0;
    command_line = GetCommandLineW();
    if (command_line == NULL) {
        return NULL;
    }
    wide.items = NULL;
    wide.count = 0;
    wide.capacity = 0;
    if (ls_rt_io_split_command_line(command_line, &wide) != 0 ||
        wide.count == 0 || wide.count > (size_t)INT_MAX) {
        ls_rt_io_wide_arguments_dispose(&wide);
        return NULL;
    }
    result = (char**)calloc(wide.count, sizeof *result);
    if (result == NULL) {
        ls_rt_io_wide_arguments_dispose(&wide);
        return NULL;
    }
    for (index = 0; index < wide.count; index += 1) {
        result[index] = ls_rt_io_utf8_from_wide_alloc(wide.items[index]);
        if (result[index] == NULL) {
            size_t freed;
            for (freed = 0; freed < index; freed += 1) {
                free(result[freed]);
            }
            free(result);
            ls_rt_io_wide_arguments_dispose(&wide);
            return NULL;
        }
    }
    count = (int)wide.count;
    ls_rt_io_wide_arguments_dispose(&wide);
    *out_argc = count;
    return result;
}

/* Convert an LsString OS text (path, address, variable name) to UTF-16. */
static wchar_t* ls_rt_io_wide_path(const LsString* path, const char* what) {
    char* text = ls_rt_io_text_copy(path, what);
    wchar_t* wide;

    if (text == NULL) {
        return NULL;
    }
    wide = ls_rt_io_wide_from_bytes(text, strlen(text));
    free(text);
    return wide;
}

/* --- Win32 error translation --- */

static int64_t ls_rt_io_kind_from_win32(DWORD code) {
    switch (code) {
        case ERROR_FILE_NOT_FOUND:
        case ERROR_PATH_NOT_FOUND:
        case ERROR_INVALID_DRIVE:
            return LS_IO_ERROR_NOT_FOUND;
        case ERROR_ACCESS_DENIED:
        case ERROR_PRIVILEGE_NOT_HELD:
        case ERROR_WRITE_PROTECT:
            return LS_IO_ERROR_PERMISSION_DENIED;
        case ERROR_ALREADY_EXISTS:
        case ERROR_FILE_EXISTS:
            return LS_IO_ERROR_ALREADY_EXISTS;
        case ERROR_INVALID_NAME:
        case ERROR_INVALID_PARAMETER:
        case ERROR_BAD_PATHNAME:
        case ERROR_DIRECTORY:
        case ERROR_INVALID_HANDLE:
        case ERROR_BUFFER_OVERFLOW:
            return LS_IO_ERROR_INVALID_INPUT;
        case ERROR_HANDLE_EOF:
            return LS_IO_ERROR_UNEXPECTED_EOF;
        case ERROR_NOT_SUPPORTED:
        case ERROR_INVALID_FUNCTION:
        case ERROR_CALL_NOT_IMPLEMENTED:
            return LS_IO_ERROR_UNSUPPORTED;
        case ERROR_BROKEN_PIPE:
        case ERROR_NO_DATA:
        case ERROR_PIPE_NOT_CONNECTED:
            return LS_IO_ERROR_BROKEN_PIPE;
        case ERROR_OPERATION_ABORTED:
            return LS_IO_ERROR_INTERRUPTED;
        case ERROR_SEM_TIMEOUT:
        case WAIT_TIMEOUT:
            return LS_IO_ERROR_TIMED_OUT;
        case WSAECONNREFUSED:
            return LS_IO_ERROR_CONNECTION_REFUSED;
        case WSAECONNRESET:
            return LS_IO_ERROR_CONNECTION_RESET;
        case WSAECONNABORTED:
            return LS_IO_ERROR_CONNECTION_ABORTED;
        case WSAETIMEDOUT:
            return LS_IO_ERROR_TIMED_OUT;
        case WSAEWOULDBLOCK:
            return LS_IO_ERROR_WOULD_BLOCK;
        case WSAEINTR:
            return LS_IO_ERROR_INTERRUPTED;
        case WSAEOPNOTSUPP:
            return LS_IO_ERROR_UNSUPPORTED;
        case WSAEADDRINUSE:
            return LS_IO_ERROR_ADDRESS_IN_USE;
        case WSAEADDRNOTAVAIL:
        case WSAEAFNOSUPPORT:
            return LS_IO_ERROR_ADDRESS_NOT_AVAILABLE;
        case WSAENOTSOCK:
        case WSAEINVAL:
            return LS_IO_ERROR_INVALID_INPUT;
        default:
            return LS_IO_ERROR_OTHER;
    }
}

/* Record a Win32 error code with its (trimmed) system message. */
static int64_t ls_rt_io_fail_win32(DWORD code) {
    char message[LS_RT_IO_INITIAL_ERROR_MESSAGE_MAX];
    wchar_t* wide = NULL;
    DWORD length = FormatMessageW(FORMAT_MESSAGE_ALLOCATE_BUFFER |
                                      FORMAT_MESSAGE_FROM_SYSTEM |
                                      FORMAT_MESSAGE_IGNORE_INSERTS,
                                  NULL, code,
                                  MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
                                  (LPWSTR)&wide, 0, NULL);

    if (length != 0 && wide != NULL) {
        LsString* text = ls_rt_io_string_from_wide(wide);
        LocalFree(wide);
        if (text != NULL) {
            const char* bytes = ls_str_data(text);
            size_t size = text->len;
            while (size > 0) {
                char last = bytes[size - 1];
                if (last != ' ' && last != '\r' && last != '\n' &&
                    last != '\t') {
                    break;
                }
                size -= 1;
            }
            (void)snprintf(message, sizeof message, "%.*s (os error %lu)",
                           (int)size, bytes, (unsigned long)code);
            ls_release(text);
            return ls_rt_io_fail(ls_rt_io_kind_from_win32(code), message);
        }
    }
    (void)snprintf(message, sizeof message, "windows error %lu",
                   (unsigned long)code);
    return ls_rt_io_fail(ls_rt_io_kind_from_win32(code), message);
}

static int64_t ls_rt_io_fail_last_win32(void) {
    return ls_rt_io_fail_win32(GetLastError());
}

static int64_t ls_rt_io_fail_wsa(int code) {
    return ls_rt_io_fail_win32((DWORD)code);
}

/* Close a native resource without touching the handle table. */
static void ls_rt_io_native_close(intptr_t native, unsigned char kind) {
    if (kind == LS_RT_IO_KIND_SOCKET) {
        (void)closesocket((SOCKET)native);
    } else if (kind == LS_RT_IO_KIND_FILE) {
        (void)CloseHandle((HANDLE)native);
    }
}

/* Clamp a remaining byte count to the DWORD range used by the Win32 calls. */
static DWORD ls_rt_io_chunk_size(int64_t remaining) {
    return remaining > (int64_t)INT_MAX ? (DWORD)INT_MAX : (DWORD)remaining;
}

/* --- Files --- */

int64_t ls_io_open(const LsString* path) {
    wchar_t* wide;
    HANDLE file;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    file = CreateFileW(wide, GENERIC_READ,
                       FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                       NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    free(wide);
    if (file == INVALID_HANDLE_VALUE) {
        return ls_rt_io_fail_last_win32();
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_FILE, (intptr_t)file);
}

int64_t ls_io_create(const LsString* path) {
    wchar_t* wide;
    HANDLE file;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    file = CreateFileW(wide, GENERIC_WRITE,
                       FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                       NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    free(wide);
    if (file == INVALID_HANDLE_VALUE) {
        return ls_rt_io_fail_last_win32();
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_FILE, (intptr_t)file);
}

int64_t ls_io_close(int64_t handle) {
    LsRtIoHandle slot;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_detach(handle, &slot)) {
        return -1;
    }
    ls_rt_io_native_close(slot.native, slot.kind);
    return 0;
}

int64_t ls_io_read(int64_t handle, void* buffer, int64_t capacity) {
    LsRtIoHandle slot;
    DWORD chunk;

    ls_rt_io_error_reset();
    if (capacity < 0) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the read capacity must not be negative");
    }
    if (capacity == 0) {
        return 0;
    }
    if (buffer == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the read buffer is null");
    }
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    chunk = ls_rt_io_chunk_size(capacity);
    switch (slot.kind) {
        case LS_RT_IO_KIND_FILE: {
            DWORD read = 0;
            if (!ReadFile((HANDLE)slot.native, buffer, chunk, &read, NULL)) {
                DWORD code = GetLastError();
                if (code == ERROR_HANDLE_EOF || code == ERROR_BROKEN_PIPE) {
                    return 0;
                }
                return ls_rt_io_fail_win32(code);
            }
            return (int64_t)read;
        }
        case LS_RT_IO_KIND_STD: {
            int read = _read((int)slot.native, buffer, (unsigned int)chunk);
            if (read < 0) {
                return ls_rt_io_fail_errno(errno);
            }
            return (int64_t)read;
        }
        default: {
            int read = recv((SOCKET)slot.native, (char*)buffer, (int)chunk, 0);
            if (read == SOCKET_ERROR) {
                return ls_rt_io_fail_wsa(WSAGetLastError());
            }
            return (int64_t)read;
        }
    }
}

int64_t ls_io_write(int64_t handle, const void* buffer, int64_t length) {
    LsRtIoHandle slot;
    int64_t total = 0;

    ls_rt_io_error_reset();
    if (length < 0) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the write length must not be negative");
    }
    if (length == 0) {
        return 0;
    }
    if (buffer == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the write buffer is null");
    }
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    while (total < length) {
        DWORD chunk = ls_rt_io_chunk_size(length - total);
        const char* bytes = (const char*)buffer + total;
        if (slot.kind == LS_RT_IO_KIND_FILE) {
            DWORD written = 0;
            if (!WriteFile((HANDLE)slot.native, bytes, chunk, &written, NULL)) {
                DWORD code = GetLastError();
                if (total > 0 && (code == ERROR_BROKEN_PIPE ||
                                  code == ERROR_NO_DATA)) {
                    return total;
                }
                return ls_rt_io_fail_win32(code);
            }
            if (written == 0) {
                break;
            }
            total += (int64_t)written;
        } else if (slot.kind == LS_RT_IO_KIND_STD) {
            int written = _write((int)slot.native, bytes, (unsigned int)chunk);
            if (written < 0) {
                return ls_rt_io_fail_errno(errno);
            }
            if (written == 0) {
                break;
            }
            total += (int64_t)written;
        } else {
            int written =
                send((SOCKET)slot.native, bytes, (int)chunk, 0);
            if (written == SOCKET_ERROR) {
                int code = WSAGetLastError();
                if (code == WSAEWOULDBLOCK && total > 0) {
                    return total;
                }
                return ls_rt_io_fail_wsa(code);
            }
            total += (int64_t)written;
        }
    }
    return total;
}

int64_t ls_io_seek(int64_t handle, int64_t offset, int32_t whence) {
    LsRtIoHandle slot;
    DWORD method;

    ls_rt_io_error_reset();
    switch (whence) {
        case 0:
            method = FILE_BEGIN;
            break;
        case 1:
            method = FILE_CURRENT;
            break;
        case 2:
            method = FILE_END;
            break;
        default:
            return ls_rt_io_fail(
                LS_IO_ERROR_INVALID_INPUT,
                "the whence value must be 0 (start), 1 (current) or 2 (end)");
    }
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_FILE | LS_RT_IO_ALLOW_STD,
                                 &slot)) {
        return -1;
    }
    if (slot.kind == LS_RT_IO_KIND_FILE) {
        LARGE_INTEGER distance;
        LARGE_INTEGER position;

        distance.QuadPart = offset;
        if (!SetFilePointerEx((HANDLE)slot.native, distance, &position,
                              method)) {
            return ls_rt_io_fail_last_win32();
        }
        return (int64_t)position.QuadPart;
    }
    {
        __int64 position = _lseeki64((int)slot.native, offset, (int)whence);

        if (position < 0) {
            return ls_rt_io_fail_errno(errno);
        }
        return (int64_t)position;
    }
}

int64_t ls_io_flush(int64_t handle) {
    LsRtIoHandle slot;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    if (slot.kind == LS_RT_IO_KIND_FILE) {
        if (!FlushFileBuffers((HANDLE)slot.native)) {
            return ls_rt_io_fail_last_win32();
        }
        return 0;
    }
    if (slot.kind == LS_RT_IO_KIND_STD) {
        if (fflush(NULL) != 0) {
            return ls_rt_io_fail_errno(errno);
        }
        return 0;
    }
    return 0;
}

int64_t ls_io_size(int64_t handle) {
    LsRtIoHandle slot;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_FILE | LS_RT_IO_ALLOW_STD,
                                 &slot)) {
        return -1;
    }
    if (slot.kind == LS_RT_IO_KIND_FILE) {
        LARGE_INTEGER size;
        if (!GetFileSizeEx((HANDLE)slot.native, &size)) {
            return ls_rt_io_fail_last_win32();
        }
        return (int64_t)size.QuadPart;
    }
    if (slot.kind == LS_RT_IO_KIND_STD) {
        struct _stat64 info;
        if (_fstat64((int)slot.native, &info) != 0) {
            return ls_rt_io_fail_errno(errno);
        }
        return (int64_t)info.st_size;
    }
    return ls_rt_io_fail_unsupported("size");
}

int64_t ls_io_set_nonblocking(int64_t handle, bool enabled) {
    LsRtIoHandle slot;
    u_long mode;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    if (slot.kind != LS_RT_IO_KIND_SOCKET) {
        return ls_rt_io_fail_unsupported("nonblocking mode");
    }
    mode = enabled ? 1u : 0u;
    if (ioctlsocket((SOCKET)slot.native, FIONBIO, &mode) != 0) {
        return ls_rt_io_fail_wsa(WSAGetLastError());
    }
    return 0;
}

/* --- Directories, metadata and process paths (Windows) --- */

static int64_t ls_rt_io_remove_leaf_windows(const wchar_t* path,
                                            DWORD attributes) {
    DWORD code;

    if ((attributes & FILE_ATTRIBUTE_DIRECTORY) != 0) {
        if (RemoveDirectoryW(path)) {
            return 0;
        }
        code = GetLastError();
        if (code == ERROR_ACCESS_DENIED) {
            (void)SetFileAttributesW(path, FILE_ATTRIBUTE_NORMAL);
            if (RemoveDirectoryW(path)) {
                return 0;
            }
            code = GetLastError();
        }
        return ls_rt_io_fail_win32(code);
    }
    if (DeleteFileW(path)) {
        return 0;
    }
    code = GetLastError();
    if (code == ERROR_ACCESS_DENIED) {
        (void)SetFileAttributesW(path, FILE_ATTRIBUTE_NORMAL);
        if (DeleteFileW(path)) {
            return 0;
        }
        code = GetLastError();
    }
    return ls_rt_io_fail_win32(code);
}

/* Remove a directory tree with an explicit stack: no recursion depth limit and
 * reparse points are removed as links instead of being followed. */
static int64_t ls_rt_io_remove_dir_tree_windows(const wchar_t* root) {
    LsRtIoNameList stack = {NULL, 0, 0};
    size_t root_length = wcslen(root);
    wchar_t* root_copy = (wchar_t*)malloc((root_length + 1) * sizeof *root_copy);
    int64_t status = 0;

    if (root_copy == NULL) {
        ls_rt_alloc_failure((root_length + 1) * sizeof *root_copy);
    }
    memcpy(root_copy, root, (root_length + 1) * sizeof *root_copy);
    ls_rt_io_name_list_push(&stack, root_copy);
    while (stack.count > 0) {
        wchar_t* current;
        DWORD attributes;

        stack.count -= 1;
        current = stack.items[stack.count];
        stack.items[stack.count] = NULL;
        attributes = GetFileAttributesW(current);
        if (attributes == INVALID_FILE_ATTRIBUTES) {
            DWORD code = GetLastError();

            if (code == ERROR_FILE_NOT_FOUND || code == ERROR_PATH_NOT_FOUND) {
                free(current);
                continue;
            }
            free(current);
            ls_rt_io_name_list_free(&stack);
            return ls_rt_io_fail_win32(code);
        }
        if ((attributes & FILE_ATTRIBUTE_DIRECTORY) != 0 &&
            (attributes & FILE_ATTRIBUTE_REPARSE_POINT) == 0) {
            size_t current_length = wcslen(current);
            wchar_t* again =
                (wchar_t*)malloc((current_length + 1) * sizeof *again);
            wchar_t* pattern =
                (wchar_t*)malloc((current_length + 3) * sizeof *pattern);
            WIN32_FIND_DATAW entry;
            HANDLE find;
            bool has_children = false;

            if (again == NULL || pattern == NULL) {
                free(again);
                free(pattern);
                free(current);
                ls_rt_io_name_list_free(&stack);
                ls_rt_alloc_failure(0);
            }
            memcpy(again, current, (current_length + 1) * sizeof *again);
            memcpy(pattern, current, current_length * sizeof *pattern);
            pattern[current_length] = L'\\';
            pattern[current_length + 1] = L'*';
            pattern[current_length + 2] = L'\0';
            find = FindFirstFileW(pattern, &entry);
            free(pattern);
            if (find == INVALID_HANDLE_VALUE) {
                DWORD code = GetLastError();

                free(again);
                if (code == ERROR_FILE_NOT_FOUND ||
                    code == ERROR_PATH_NOT_FOUND) {
                    /* The directory is empty: it is now a leaf. */
                    status = ls_rt_io_remove_leaf_windows(current, attributes);
                    free(current);
                    if (status != 0) {
                        ls_rt_io_name_list_free(&stack);
                        return -1;
                    }
                    continue;
                }
                free(current);
                ls_rt_io_name_list_free(&stack);
                return ls_rt_io_fail_win32(code);
            }
            ls_rt_io_name_list_push(&stack, again);
            do {
                size_t name_length;
                wchar_t* child;

                if (wcscmp(entry.cFileName, L".") == 0 ||
                    wcscmp(entry.cFileName, L"..") == 0) {
                    continue;
                }
                name_length = wcslen(entry.cFileName);
                child = (wchar_t*)malloc(
                    (current_length + name_length + 2) * sizeof *child);
                if (child == NULL) {
                    (void)FindClose(find);
                    free(current);
                    ls_rt_io_name_list_free(&stack);
                    ls_rt_alloc_failure(0);
                }
                memcpy(child, current, current_length * sizeof *child);
                child[current_length] = L'\\';
                memcpy(child + current_length + 1, entry.cFileName,
                       (name_length + 1) * sizeof *child);
                ls_rt_io_name_list_push(&stack, child);
                has_children = true;
            } while (FindNextFileW(find, &entry));
            {
                DWORD code = GetLastError();

                (void)FindClose(find);
                if (code != ERROR_NO_MORE_FILES) {
                    free(current);
                    ls_rt_io_name_list_free(&stack);
                    return ls_rt_io_fail_win32(code);
                }
            }
            if (!has_children) {
                /* The directory is empty: discard its "again" entry (the last
                 * stack slot) and remove the directory as a leaf. */
                stack.count -= 1;
                free(stack.items[stack.count]);
                stack.items[stack.count] = NULL;
                status = ls_rt_io_remove_leaf_windows(current, attributes);
                free(current);
                if (status != 0) {
                    ls_rt_io_name_list_free(&stack);
                    return -1;
                }
                continue;
            }
            free(current);
            continue;
        }
        status = ls_rt_io_remove_leaf_windows(current, attributes);
        free(current);
        if (status != 0) {
            ls_rt_io_name_list_free(&stack);
            return -1;
        }
    }
    ls_rt_io_name_list_free(&stack);
    return 0;
}

LsVec_String* ls_io_read_dir(const LsString* path) {
    wchar_t* wide;
    wchar_t* pattern;
    size_t path_length;
    WIN32_FIND_DATAW entry;
    HANDLE find;
    LsRtIoNameList names = {NULL, 0, 0};
    LsVec_String* result;
    size_t index;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return NULL;
    }
    path_length = wcslen(wide);
    pattern = (wchar_t*)malloc((path_length + 3) * sizeof *pattern);
    if (pattern == NULL) {
        free(wide);
        ls_rt_alloc_failure((path_length + 3) * sizeof *pattern);
    }
    memcpy(pattern, wide, path_length * sizeof *pattern);
    if (path_length != 0 && (wide[path_length - 1] == L'\\' ||
                             wide[path_length - 1] == L'/')) {
        pattern[path_length] = L'*';
        pattern[path_length + 1] = L'\0';
    } else {
        pattern[path_length] = L'\\';
        pattern[path_length + 1] = L'*';
        pattern[path_length + 2] = L'\0';
    }
    find = FindFirstFileW(pattern, &entry);
    free(pattern);
    if (find == INVALID_HANDLE_VALUE) {
        DWORD code = GetLastError();
        DWORD attributes;
        bool directory;

        attributes = GetFileAttributesW(wide);
        directory = attributes != INVALID_FILE_ATTRIBUTES &&
                    (attributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
        if (code == ERROR_FILE_NOT_FOUND && directory) {
            /* The directory exists but is empty. */
            free(wide);
            result = ls_rt_io_vec_string_new();
            return result;
        }
        free(wide);
        if (attributes != INVALID_FILE_ATTRIBUTES && !directory) {
            (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                                "the path is not a directory");
            return NULL;
        }
        (void)ls_rt_io_fail_win32(code);
        return NULL;
    }
    free(wide);
    do {
        size_t name_length;
        wchar_t* name;

        if (wcscmp(entry.cFileName, L".") == 0 ||
            wcscmp(entry.cFileName, L"..") == 0) {
            continue;
        }
        name_length = wcslen(entry.cFileName);
        name = (wchar_t*)malloc((name_length + 1) * sizeof *name);
        if (name == NULL) {
            (void)FindClose(find);
            ls_rt_io_name_list_free(&names);
            ls_rt_alloc_failure(0);
        }
        memcpy(name, entry.cFileName, (name_length + 1) * sizeof *name);
        ls_rt_io_name_list_push(&names, name);
    } while (FindNextFileW(find, &entry));
    {
        DWORD code = GetLastError();

        (void)FindClose(find);
        if (code != ERROR_NO_MORE_FILES) {
            ls_rt_io_name_list_free(&names);
            (void)ls_rt_io_fail_win32(code);
            return NULL;
        }
    }
    ls_rt_io_name_list_sort(&names);
    result = ls_rt_io_vec_string_new();
    for (index = 0; index < names.count; index += 1) {
        LsString* name = ls_rt_io_string_from_wide(names.items[index]);

        if (name == NULL) {
            ls_release(result);
            ls_rt_io_name_list_free(&names);
            return NULL;
        }
        ls_vec_string_push_owned(result, name);
    }
    ls_rt_io_name_list_free(&names);
    return result;
}

int64_t ls_io_metadata(const LsString* path, int64_t* size, int64_t* mtime_ms,
                       int64_t* kind) {
    wchar_t* wide;
    WIN32_FILE_ATTRIBUTE_DATA data;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    if (!GetFileAttributesExW(wide, GetFileExInfoStandard, &data)) {
        free(wide);
        return ls_rt_io_fail_last_win32();
    }
    free(wide);
    if (size != NULL) {
        *size = ((int64_t)data.nFileSizeHigh << 32) |
                (int64_t)(uint32_t)data.nFileSizeLow;
    }
    if (mtime_ms != NULL) {
        ULARGE_INTEGER ticks;

        ticks.LowPart = data.ftLastWriteTime.dwLowDateTime;
        ticks.HighPart = data.ftLastWriteTime.dwHighDateTime;
        *mtime_ms =
            (int64_t)((ticks.QuadPart - 116444736000000000ULL) / 10000ULL);
    }
    if (kind != NULL) {
        if ((data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0) {
            *kind = 1;
        } else if ((data.dwFileAttributes & FILE_ATTRIBUTE_DEVICE) != 0) {
            *kind = 2;
        } else {
            *kind = 0;
        }
    }
    return 0;
}

int64_t ls_io_remove_file(const LsString* path) {
    wchar_t* wide;
    int64_t status;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    if (!DeleteFileW(wide)) {
        status = ls_rt_io_fail_last_win32();
        free(wide);
        return status;
    }
    free(wide);
    return 0;
}

int64_t ls_io_remove_dir(const LsString* path) {
    wchar_t* wide;
    int64_t status;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    if (!RemoveDirectoryW(wide)) {
        status = ls_rt_io_fail_last_win32();
        free(wide);
        return status;
    }
    free(wide);
    return 0;
}

int64_t ls_io_remove_dir_all(const LsString* path) {
    wchar_t* wide;
    DWORD attributes;
    int64_t status;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    attributes = GetFileAttributesW(wide);
    if (attributes == INVALID_FILE_ATTRIBUTES) {
        status = ls_rt_io_fail_last_win32();
        free(wide);
        return status;
    }
    if ((attributes & FILE_ATTRIBUTE_DIRECTORY) == 0) {
        free(wide);
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the path is not a directory");
    }
    status = ls_rt_io_remove_dir_tree_windows(wide);
    free(wide);
    return status;
}

/* Create one directory; an existing directory counts as success. */
static int64_t ls_rt_io_create_directory_tolerant(const wchar_t* path) {
    if (CreateDirectoryW(path, NULL)) {
        return 0;
    }
    {
        DWORD code = GetLastError();

        if (code == ERROR_ALREADY_EXISTS) {
            DWORD attributes = GetFileAttributesW(path);

            if (attributes != INVALID_FILE_ATTRIBUTES &&
                (attributes & FILE_ATTRIBUTE_DIRECTORY) != 0) {
                return 0;
            }
        }
        return ls_rt_io_fail_win32(code);
    }
}

int64_t ls_io_create_dir(const LsString* path) {
    wchar_t* wide;
    int64_t status;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    if (!CreateDirectoryW(wide, NULL)) {
        status = ls_rt_io_fail_last_win32();
        free(wide);
        return status;
    }
    free(wide);
    return 0;
}

int64_t ls_io_create_dir_all(const LsString* path) {
    wchar_t* wide;
    size_t length;
    size_t index;
    wchar_t* prefix;
    int64_t status = 0;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    length = wcslen(wide);
    if (length == 0) {
        free(wide);
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT, "the path is empty");
    }
    prefix = (wchar_t*)malloc((length + 1) * sizeof *prefix);
    if (prefix == NULL) {
        free(wide);
        ls_rt_alloc_failure((length + 1) * sizeof *prefix);
    }
    for (index = 1; index < length; index += 1) {
        if (wide[index] != L'\\' && wide[index] != L'/') {
            continue;
        }
        if (wide[index - 1] == L'\\' || wide[index - 1] == L'/') {
            continue;
        }
        if (index == 2 && wide[1] == L':') {
            continue;
        }
        memcpy(prefix, wide, index * sizeof *prefix);
        prefix[index] = L'\0';
        status = ls_rt_io_create_directory_tolerant(prefix);
        if (status != 0) {
            free(prefix);
            free(wide);
            return -1;
        }
    }
    memcpy(prefix, wide, (length + 1) * sizeof *prefix);
    status = ls_rt_io_create_directory_tolerant(prefix);
    free(prefix);
    free(wide);
    return status;
}

int64_t ls_io_rename(const LsString* from, const LsString* to) {
    wchar_t* wide_from;
    wchar_t* wide_to;
    int64_t status;

    ls_rt_io_error_reset();
    wide_from = ls_rt_io_wide_path(from, "path");
    if (wide_from == NULL) {
        return -1;
    }
    wide_to = ls_rt_io_wide_path(to, "path");
    if (wide_to == NULL) {
        free(wide_from);
        return -1;
    }
    if (!MoveFileExW(wide_from, wide_to, MOVEFILE_COPY_ALLOWED)) {
        status = ls_rt_io_fail_last_win32();
        free(wide_from);
        free(wide_to);
        return status;
    }
    free(wide_from);
    free(wide_to);
    return 0;
}

LsString* ls_io_canonicalize(const LsString* path) {
    wchar_t* wide;
    HANDLE file;
    DWORD required;
    wchar_t* buffer;
    DWORD written;
    const wchar_t* stripped;
    wchar_t* unc_copy = NULL;
    LsString* result;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return NULL;
    }
    file = CreateFileW(wide, FILE_READ_ATTRIBUTES,
                       FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                       NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL);
    free(wide);
    if (file == INVALID_HANDLE_VALUE) {
        (void)ls_rt_io_fail_last_win32();
        return NULL;
    }
    required = GetFinalPathNameByHandleW(
        file, NULL, 0, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
    if (required == 0) {
        (void)ls_rt_io_fail_last_win32();
        (void)CloseHandle(file);
        return NULL;
    }
    buffer = (wchar_t*)malloc(((size_t)required + 1) * sizeof *buffer);
    if (buffer == NULL) {
        (void)CloseHandle(file);
        ls_rt_alloc_failure(((size_t)required + 1) * sizeof *buffer);
    }
    written = GetFinalPathNameByHandleW(
        file, buffer, required + 1, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
    (void)CloseHandle(file);
    if (written == 0) {
        free(buffer);
        (void)ls_rt_io_fail_last_win32();
        return NULL;
    }
    buffer[written < required ? written : required] = L'\0';
    stripped = buffer;
    if (wcsncmp(buffer, L"\\\\?\\UNC\\", 8) == 0) {
        size_t rest = wcslen(buffer + 8);

        unc_copy = (wchar_t*)malloc((rest + 3) * sizeof *unc_copy);
        if (unc_copy == NULL) {
            free(buffer);
            ls_rt_alloc_failure((rest + 3) * sizeof *unc_copy);
        }
        unc_copy[0] = L'\\';
        unc_copy[1] = L'\\';
        memcpy(unc_copy + 2, buffer + 8, (rest + 1) * sizeof *unc_copy);
        stripped = unc_copy;
    } else if (wcsncmp(buffer, L"\\\\?\\", 4) == 0) {
        stripped = buffer + 4;
    }
    result = ls_rt_io_string_from_wide(stripped);
    free(unc_copy);
    free(buffer);
    return result;
}

LsString* ls_io_current_dir(void) {
    DWORD capacity = GetCurrentDirectoryW(0, NULL);
    int attempts;

    ls_rt_io_error_reset();
    if (capacity == 0) {
        (void)ls_rt_io_fail_last_win32();
        return NULL;
    }
    for (attempts = 0; attempts < 4; attempts += 1) {
        wchar_t* buffer =
            (wchar_t*)malloc((size_t)capacity * sizeof *buffer);
        DWORD written;

        if (buffer == NULL) {
            ls_rt_alloc_failure((size_t)capacity * sizeof *buffer);
        }
        written = GetCurrentDirectoryW(capacity, buffer);
        if (written == 0) {
            DWORD code = GetLastError();

            free(buffer);
            (void)ls_rt_io_fail_win32(code);
            return NULL;
        }
        if (written < capacity) {
            LsString* result = ls_rt_io_string_from_wide(buffer);

            free(buffer);
            return result;
        }
        free(buffer);
        capacity = written + 1;
    }
    (void)ls_rt_io_fail(LS_IO_ERROR_OTHER,
                        "the current directory path changed while reading it");
    return NULL;
}

int64_t ls_io_set_current_dir(const LsString* path) {
    wchar_t* wide;
    int64_t status;

    ls_rt_io_error_reset();
    wide = ls_rt_io_wide_path(path, "path");
    if (wide == NULL) {
        return -1;
    }
    if (!SetCurrentDirectoryW(wide)) {
        status = ls_rt_io_fail_last_win32();
        free(wide);
        return status;
    }
    free(wide);
    return 0;
}

LsString* ls_io_temp_dir(void) {
    DWORD capacity = GetTempPathW(0, NULL);
    int attempts;

    ls_rt_io_error_reset();
    if (capacity == 0) {
        (void)ls_rt_io_fail_last_win32();
        return NULL;
    }
    for (attempts = 0; attempts < 4; attempts += 1) {
        wchar_t* buffer =
            (wchar_t*)malloc((size_t)capacity * sizeof *buffer);
        DWORD written;

        if (buffer == NULL) {
            ls_rt_alloc_failure((size_t)capacity * sizeof *buffer);
        }
        written = GetTempPathW(capacity, buffer);
        if (written == 0) {
            DWORD code = GetLastError();

            free(buffer);
            (void)ls_rt_io_fail_win32(code);
            return NULL;
        }
        if (written < capacity) {
            LsString* result;

            while (written > 1 &&
                   (buffer[written - 1] == L'\\' ||
                    buffer[written - 1] == L'/')) {
                if (written == 3 && buffer[1] == L':') {
                    break;
                }
                written -= 1;
            }
            buffer[written] = L'\0';
            result = ls_rt_io_string_from_wide(buffer);
            free(buffer);
            return result;
        }
        free(buffer);
        capacity = written + 1;
    }
    (void)ls_rt_io_fail(LS_IO_ERROR_OTHER,
                        "the temporary directory path changed while reading it");
    return NULL;
}


/* --- TCP sockets (Windows) --- */

int64_t ls_net_listen(const LsString* address) {
    struct addrinfo* results = NULL;
    struct addrinfo* candidate;
    SOCKET listener = INVALID_SOCKET;
    int last_error = 0;

    ls_rt_io_error_reset();
    if (ls_rt_io_resolve_address(address, true, &results) != 0) {
        return -1;
    }
    for (candidate = results; candidate != NULL; candidate = candidate->ai_next) {
        SOCKET socket_value = socket(candidate->ai_family, candidate->ai_socktype,
                                     candidate->ai_protocol);
        int reuse = 1;

        if (socket_value == INVALID_SOCKET) {
            last_error = WSAGetLastError();
            continue;
        }
        (void)setsockopt(socket_value, SOL_SOCKET, SO_REUSEADDR,
                         (const char*)&reuse, (int)sizeof reuse);
        if (bind(socket_value, candidate->ai_addr, (int)candidate->ai_addrlen) == 0 &&
            listen(socket_value, SOMAXCONN) == 0) {
            listener = socket_value;
            break;
        }
        last_error = WSAGetLastError();
        (void)closesocket(socket_value);
    }
    freeaddrinfo(results);
    if (listener == INVALID_SOCKET) {
        if (last_error == 0) {
            return ls_rt_io_fail(LS_IO_ERROR_ADDRESS_NOT_AVAILABLE,
                                 "the address could not be bound");
        }
        return ls_rt_io_fail_wsa(last_error);
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_SOCKET, (intptr_t)listener);
}

int64_t ls_net_local_port(int64_t handle) {
    LsRtIoHandle slot;
    struct sockaddr_storage address;
    int length = (int)sizeof address;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    if (getsockname((SOCKET)slot.native, (struct sockaddr*)&address, &length) != 0) {
        return ls_rt_io_fail_wsa(WSAGetLastError());
    }
    if (address.ss_family == AF_INET) {
        return (int64_t)ntohs(((struct sockaddr_in*)&address)->sin_port);
    }
    if (address.ss_family == AF_INET6) {
        return (int64_t)ntohs(((struct sockaddr_in6*)&address)->sin6_port);
    }
    return ls_rt_io_fail(LS_IO_ERROR_OTHER,
                         "the socket has an unsupported address family");
}

int64_t ls_net_accept(int64_t listener) {
    LsRtIoHandle slot;
    SOCKET connection;
    int code;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(listener, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    connection = accept((SOCKET)slot.native, NULL, NULL);
    if (connection == INVALID_SOCKET) {
        code = WSAGetLastError();
        if (code == WSAEWOULDBLOCK) {
            return ls_rt_io_fail(LS_IO_ERROR_WOULD_BLOCK,
                                 "the operation would block");
        }
        return ls_rt_io_fail_wsa(code);
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_SOCKET, (intptr_t)connection);
}

int64_t ls_net_connect(const LsString* address) {
    struct addrinfo* results = NULL;
    struct addrinfo* candidate;
    SOCKET connection = INVALID_SOCKET;
    int last_error = 0;

    ls_rt_io_error_reset();
    if (ls_rt_io_resolve_address(address, false, &results) != 0) {
        return -1;
    }
    for (candidate = results; candidate != NULL; candidate = candidate->ai_next) {
        SOCKET socket_value = socket(candidate->ai_family, candidate->ai_socktype,
                                     candidate->ai_protocol);

        if (socket_value == INVALID_SOCKET) {
            last_error = WSAGetLastError();
            continue;
        }
        if (connect(socket_value, candidate->ai_addr,
                    (int)candidate->ai_addrlen) == 0) {
            connection = socket_value;
            break;
        }
        last_error = WSAGetLastError();
        (void)closesocket(socket_value);
    }
    freeaddrinfo(results);
    if (connection == INVALID_SOCKET) {
        if (last_error == 0) {
            return ls_rt_io_fail(LS_IO_ERROR_OTHER,
                                 "no address was available for the connection");
        }
        return ls_rt_io_fail_wsa(last_error);
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_SOCKET, (intptr_t)connection);
}

int64_t ls_net_set_timeout(int64_t handle, int64_t milliseconds) {
    LsRtIoHandle slot;
    DWORD value;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    if (milliseconds < 0) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the timeout must not be negative");
    }
    value = milliseconds > 0x7FFFFFFF ? 0x7FFFFFFF : (DWORD)milliseconds;
    if (setsockopt((SOCKET)slot.native, SOL_SOCKET, SO_RCVTIMEO,
                   (const char*)&value, (int)sizeof value) != 0) {
        return ls_rt_io_fail_wsa(WSAGetLastError());
    }
    if (setsockopt((SOCKET)slot.native, SOL_SOCKET, SO_SNDTIMEO,
                   (const char*)&value, (int)sizeof value) != 0) {
        return ls_rt_io_fail_wsa(WSAGetLastError());
    }
    return 0;
}

int64_t ls_net_shutdown(int64_t handle, int32_t how) {
    LsRtIoHandle slot;
    int mode;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    if (how == 0) {
        mode = SD_RECEIVE;
    } else if (how == 1) {
        mode = SD_SEND;
    } else if (how == 2) {
        mode = SD_BOTH;
    } else {
        return ls_rt_io_fail(
            LS_IO_ERROR_INVALID_INPUT,
            "the shutdown mode must be 0 (read), 1 (write) or 2 (both)");
    }
    if (shutdown((SOCKET)slot.native, mode) != 0) {
        return ls_rt_io_fail_wsa(WSAGetLastError());
    }
    return 0;
}

/* --- Time (Windows) --- */

#define LS_RT_IO_FILETIME_UNIX_EPOCH ((uint64_t)116444736000000000ULL)

static int64_t ls_rt_io_filetime_to_unix_ms(FILETIME value) {
    ULARGE_INTEGER ticks;

    ticks.LowPart = value.dwLowDateTime;
    ticks.HighPart = value.dwHighDateTime;
    return (int64_t)((ticks.QuadPart - LS_RT_IO_FILETIME_UNIX_EPOCH) / 10000ULL);
}

int64_t ls_time_now_ms(void) {
    FILETIME value;

    GetSystemTimeAsFileTime(&value);
    return ls_rt_io_filetime_to_unix_ms(value);
}

int64_t ls_time_monotonic_ns(void) {
    LARGE_INTEGER counter;
    LARGE_INTEGER frequency;
    int64_t seconds;
    int64_t remainder;

    if (!QueryPerformanceCounter(&counter) ||
        !QueryPerformanceFrequency(&frequency) || frequency.QuadPart <= 0) {
        return 0;
    }
    seconds = (int64_t)(counter.QuadPart / frequency.QuadPart);
    remainder = (int64_t)(counter.QuadPart % frequency.QuadPart);
    return seconds * 1000000000LL +
           (remainder * 1000000000LL) / (int64_t)frequency.QuadPart;
}

void ls_time_sleep_ms(int64_t milliseconds) {
    int64_t remaining = milliseconds;

    if (remaining <= 0) {
        (void)Sleep(0);
        return;
    }
    while (remaining > 0) {
        DWORD chunk = remaining > 0x7FFFFFFFLL ? 0x7FFFFFFF : (DWORD)remaining;

        (void)Sleep(chunk);
        remaining -= (int64_t)chunk;
    }
}

static int64_t ls_rt_io_time_parts(struct tm* values,
                                   int32_t* year, int32_t* month, int32_t* day,
                                   int32_t* hour, int32_t* minute,
                                   int32_t* second, int32_t* weekday,
                                   int32_t* yearday) {
    if (values == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the timestamp could not be converted");
    }
    if (year != NULL) *year = values->tm_year + 1900;
    if (month != NULL) *month = values->tm_mon + 1;
    if (day != NULL) *day = values->tm_mday;
    if (hour != NULL) *hour = values->tm_hour;
    if (minute != NULL) *minute = values->tm_min;
    if (second != NULL) *second = values->tm_sec;
    if (weekday != NULL) *weekday = values->tm_wday;
    if (yearday != NULL) *yearday = values->tm_yday;
    return 0;
}

int64_t ls_time_local_parts(int64_t unix_ms, int32_t* year, int32_t* month,
                            int32_t* day, int32_t* hour, int32_t* minute,
                            int32_t* second, int32_t* weekday,
                            int32_t* yearday) {
    int64_t seconds = unix_ms / 1000;
    time_t value;
    struct tm parts;
    struct tm* converted;

    ls_rt_io_error_reset();
    if (unix_ms < 0 && unix_ms % 1000 != 0) {
        seconds -= 1;
    }
    value = (time_t)seconds;
    if ((int64_t)value != seconds) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the timestamp is out of range");
    }
    converted = localtime(&value);
    if (converted == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the local time could not be computed");
    }
    parts = *converted;
    return ls_rt_io_time_parts(&parts, year, month, day, hour, minute,
                               second, weekday, yearday);
}

int64_t ls_time_utc_parts(int64_t unix_ms, int32_t* year, int32_t* month,
                          int32_t* day, int32_t* hour, int32_t* minute,
                          int32_t* second, int32_t* weekday,
                          int32_t* yearday) {
    int64_t seconds = unix_ms / 1000;
    time_t value;
    struct tm parts;
    struct tm* converted;

    ls_rt_io_error_reset();
    if (unix_ms < 0 && unix_ms % 1000 != 0) {
        seconds -= 1;
    }
    value = (time_t)seconds;
    if ((int64_t)value != seconds) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the timestamp is out of range");
    }
    converted = gmtime(&value);
    if (converted == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the UTC time could not be computed");
    }
    parts = *converted;
    return ls_rt_io_time_parts(&parts, year, month, day, hour, minute,
                               second, weekday, yearday);
}

/* --- Environment --- */

LsString* ls_env_var(const LsString* name) {
    char* name_text;
    wchar_t* wide;
    DWORD capacity;
    DWORD written;
    int attempts;
    wchar_t* value = NULL;
    LsString* result;

    ls_rt_io_error_reset();
    name_text = ls_rt_io_text_copy(name, "name");
    if (name_text == NULL) {
        return NULL;
    }
    wide = ls_rt_io_wide_from_bytes(name_text, strlen(name_text));
    free(name_text);
    if (wide == NULL) {
        return NULL;
    }
    SetLastError(ERROR_SUCCESS);
    capacity = GetEnvironmentVariableW(wide, NULL, 0);
    if (capacity == 0) {
        DWORD code = GetLastError();

        free(wide);
        if (code == ERROR_ENVVAR_NOT_FOUND) {
            return NULL;
        }
        if (code == ERROR_SUCCESS) {
            return ls_str_alloc_from("", 0);
        }
        (void)ls_rt_io_fail_win32(code);
        return NULL;
    }
    for (attempts = 0; attempts < 4; attempts += 1) {
        DWORD code;

        value = (wchar_t*)malloc((size_t)capacity * sizeof *value);
        if (value == NULL) {
            free(wide);
            ls_rt_alloc_failure((size_t)capacity * sizeof *value);
        }
        SetLastError(ERROR_SUCCESS);
        written = GetEnvironmentVariableW(wide, value, capacity);
        if (written == 0) {
            code = GetLastError();
            if (code == ERROR_ENVVAR_NOT_FOUND) {
                free(value);
                free(wide);
                return NULL;
            }
            if (code == ERROR_SUCCESS) {
                value[0] = L'\0';
                break;
            }
            free(value);
            free(wide);
            (void)ls_rt_io_fail_win32(code);
            return NULL;
        }
        if (written < capacity) {
            break;
        }
        free(value);
        value = NULL;
        capacity = written + 1;
    }
    free(wide);
    if (value == NULL) {
        (void)ls_rt_io_fail(LS_IO_ERROR_OTHER,
                            "the environment variable is too long");
        return NULL;
    }
    result = ls_rt_io_string_from_wide(value);
    free(value);
    return result;
}

LsVec_String* ls_env_vars(void) {
    wchar_t* block;
    wchar_t* entry;
    LsVec_String* result;

    ls_rt_io_error_reset();
    block = GetEnvironmentStringsW();
    if (block == NULL) {
        (void)ls_rt_io_fail_last_win32();
        return NULL;
    }
    result = ls_rt_io_vec_string_new();
    entry = block;
    while (*entry != L'\0') {
        size_t length = wcslen(entry);

        if (entry[0] != L'=') {
            LsString* item = ls_rt_io_string_from_wide(entry);

            if (item == NULL) {
                (void)FreeEnvironmentStringsW(block);
                ls_release(result);
                return NULL;
            }
            ls_vec_string_push_owned(result, item);
        }
        entry += length + 1;
    }
    (void)FreeEnvironmentStringsW(block);
    if (result->len > 1) {
        qsort(result->data, result->len, sizeof *result->data,
              ls_rt_io_string_compare);
    }
    return result;
}

/* --- Platform lifetime (Windows) --- */

static int ls_rt_io_wsa_started = 0;

static int ls_rt_io_platform_init(void) {
    WSADATA data;

    if (ls_rt_io_wsa_started) {
        return 0;
    }
    if (WSAStartup(MAKEWORD(2, 2), &data) != 0) {
        return -1;
    }
    ls_rt_io_wsa_started = 1;
    return 0;
}

static void ls_rt_io_platform_shutdown(void) {
    if (ls_rt_io_wsa_started) {
        (void)WSACleanup();
        ls_rt_io_wsa_started = 0;
    }
}

#else /* POSIX implementation */

/* --- POSIX files --- */

static void ls_rt_io_close_descriptor(intptr_t native) {
    if (native >= 0) {
        (void)close((int)native);
    }
}

/* Close a native resource without touching the handle table. */
static void ls_rt_io_native_close(intptr_t native, unsigned char kind) {
    (void)kind;
    ls_rt_io_close_descriptor(native);
}

/* Build a fresh owned string from bytes that are known to be valid UTF-8. */
static LsString* ls_rt_io_string_from_valid(const char* bytes, size_t len) {
    return ls_str_alloc_from(bytes, len);
}

static LsString* ls_rt_io_string_from_path_bytes(const char* bytes) {
    size_t length = strlen(bytes);

    if (!ls_rt_utf8_validate(bytes, length)) {
        (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                            "the path is not valid UTF-8");
        return NULL;
    }
    return ls_rt_io_string_from_valid(bytes, length);
}

int64_t ls_io_open(const LsString* path) {
    char* text;
    int descriptor;
    int64_t handle;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    do {
        descriptor = open(text, O_RDONLY);
    } while (descriptor < 0 && errno == EINTR);
    free(text);
    if (descriptor < 0) {
        return ls_rt_io_fail_errno(errno);
    }
    handle = ls_rt_io_handle_allocate(LS_RT_IO_KIND_FILE, (intptr_t)descriptor);
    return handle;
}

int64_t ls_io_create(const LsString* path) {
    char* text;
    int descriptor;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    do {
        descriptor = open(text, O_WRONLY | O_CREAT | O_TRUNC, 0666);
    } while (descriptor < 0 && errno == EINTR);
    free(text);
    if (descriptor < 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_FILE, (intptr_t)descriptor);
}

int64_t ls_io_close(int64_t handle) {
    LsRtIoHandle slot;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_detach(handle, &slot)) {
        return -1;
    }
    ls_rt_io_close_descriptor(slot.native);
    return 0;
}

int64_t ls_io_read(int64_t handle, void* buffer, int64_t capacity) {
    LsRtIoHandle slot;
    int64_t total = 0;

    ls_rt_io_error_reset();
    if (capacity < 0) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the read capacity must not be negative");
    }
    if (capacity == 0) {
        return 0;
    }
    if (buffer == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the read buffer is null");
    }
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    if (slot.kind == LS_RT_IO_KIND_SOCKET) {
        ssize_t got;

        do {
            got = recv((int)slot.native, buffer, (size_t)capacity, 0);
        } while (got < 0 && errno == EINTR);
        if (got < 0) {
            return ls_rt_io_fail_errno(errno);
        }
        return (int64_t)got;
    }
    while (total < capacity) {
        ssize_t got;

        do {
            got = read((int)slot.native, (char*)buffer + total,
                       (size_t)(capacity - total));
        } while (got < 0 && errno == EINTR);
        if (got < 0) {
            if (total > 0) {
                return total;
            }
            return ls_rt_io_fail_errno(errno);
        }
        if (got == 0) {
            break;
        }
        total += (int64_t)got;
        if (slot.kind != LS_RT_IO_KIND_FILE) {
            break;
        }
    }
    return total;
}

int64_t ls_io_write(int64_t handle, const void* buffer, int64_t length) {
    LsRtIoHandle slot;
    int64_t total = 0;

    ls_rt_io_error_reset();
    if (length < 0) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the write length must not be negative");
    }
    if (length == 0) {
        return 0;
    }
    if (buffer == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the write buffer is null");
    }
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    while (total < length) {
        ssize_t written;

        if (slot.kind == LS_RT_IO_KIND_SOCKET) {
            written = send((int)slot.native, (const char*)buffer + total,
                           (size_t)(length - total), 0);
        } else {
            written = write((int)slot.native, (const char*)buffer + total,
                            (size_t)(length - total));
        }
        if (written < 0) {
            if (errno == EINTR) {
                continue;
            }
            if (total > 0) {
                return total;
            }
            return ls_rt_io_fail_errno(errno);
        }
        if (written == 0) {
            break;
        }
        total += (int64_t)written;
    }
    return total;
}

int64_t ls_io_seek(int64_t handle, int64_t offset, int32_t whence) {
    LsRtIoHandle slot;
    int mode;
    off_t position;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle,
                                 LS_RT_IO_ALLOW_FILE | LS_RT_IO_ALLOW_STD,
                                 &slot)) {
        return -1;
    }
    if (whence == 0) {
        mode = SEEK_SET;
    } else if (whence == 1) {
        mode = SEEK_CUR;
    } else if (whence == 2) {
        mode = SEEK_END;
    } else {
        return ls_rt_io_fail(
            LS_IO_ERROR_INVALID_INPUT,
            "the whence value must be 0 (start), 1 (current) or 2 (end)");
    }
    position = lseek((int)slot.native, (off_t)offset, mode);
    if (position < 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return (int64_t)position;
}

int64_t ls_io_flush(int64_t handle) {
    LsRtIoHandle slot;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    if (slot.kind == LS_RT_IO_KIND_FILE) {
        if (fsync((int)slot.native) != 0) {
            if (errno == EINVAL || errno == ENOTSUP) {
                return 0;
            }
            return ls_rt_io_fail_errno(errno);
        }
        return 0;
    }
    if (slot.kind == LS_RT_IO_KIND_STD) {
        if (fflush(NULL) != 0) {
            return ls_rt_io_fail_errno(errno);
        }
        return 0;
    }
    return 0;
}

int64_t ls_io_size(int64_t handle) {
    LsRtIoHandle slot;
    struct stat info;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle,
                                 LS_RT_IO_ALLOW_FILE | LS_RT_IO_ALLOW_STD,
                                 &slot)) {
        return -1;
    }
    if (slot.kind == LS_RT_IO_KIND_SOCKET) {
        return ls_rt_io_fail_unsupported("size");
    }
    if (fstat((int)slot.native, &info) != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return (int64_t)info.st_size;
}

int64_t ls_io_set_nonblocking(int64_t handle, bool enabled) {
    LsRtIoHandle slot;
    int flags;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_STREAM, &slot)) {
        return -1;
    }
    if (slot.kind != LS_RT_IO_KIND_SOCKET) {
        return ls_rt_io_fail_unsupported("nonblocking mode");
    }
    flags = fcntl((int)slot.native, F_GETFL);
    if (flags < 0) {
        return ls_rt_io_fail_errno(errno);
    }
    if (enabled) {
        flags |= O_NONBLOCK;
    } else {
        flags &= ~O_NONBLOCK;
    }
    if (fcntl((int)slot.native, F_SETFL, flags) < 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

LsVec_String* ls_io_read_dir(const LsString* path) {
    char* text;
    DIR* directory;
    LsRtIoNameList names = {NULL, 0, 0};
    LsVec_String* result;
    size_t index;
    int saved_errno = 0;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return NULL;
    }
    directory = opendir(text);
    if (directory == NULL) {
        int code = errno;

        free(text);
        if (code == ENOTDIR) {
            /* Match the Windows implementation and the documented contract:
             * a path that exists but is not a directory is invalid input, not
             * a missing path. */
            (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                                "the path is not a directory");
            return NULL;
        }
        (void)ls_rt_io_fail_errno(code);
        return NULL;
    }
    free(text);
    for (;;) {
        struct dirent* entry;

        errno = 0;
        entry = readdir(directory);
        if (entry == NULL) {
            saved_errno = errno;
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 ||
            strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        {
            size_t name_length = strlen(entry->d_name);
            char* name = (char*)malloc(name_length + 1);

            if (name == NULL) {
                (void)closedir(directory);
                ls_rt_io_name_list_free(&names);
                ls_rt_alloc_failure(name_length + 1);
            }
            memcpy(name, entry->d_name, name_length + 1);
            ls_rt_io_name_list_push(&names, name);
        }
    }
    (void)closedir(directory);
    if (saved_errno != 0) {
        ls_rt_io_name_list_free(&names);
        (void)ls_rt_io_fail_errno(saved_errno);
        return NULL;
    }
    ls_rt_io_name_list_sort(&names);
    result = ls_rt_io_vec_string_new();
    for (index = 0; index < names.count; index += 1) {
        size_t name_length = strlen(names.items[index]);
        LsString* name;

        if (!ls_rt_utf8_validate(names.items[index], name_length)) {
            ls_release(result);
            ls_rt_io_name_list_free(&names);
            (void)ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                                "a directory entry name is not valid UTF-8");
            return NULL;
        }
        name = ls_rt_io_string_from_valid(names.items[index], name_length);
        if (name == NULL) {
            ls_release(result);
            ls_rt_io_name_list_free(&names);
            return NULL;
        }
        ls_vec_string_push_owned(result, name);
    }
    ls_rt_io_name_list_free(&names);
    return result;
}

#if defined(__APPLE__)
#define LS_RT_IO_MTIME(info) ((info).st_mtimespec)
#else
#define LS_RT_IO_MTIME(info) ((info).st_mtim)
#endif

int64_t ls_io_metadata(const LsString* path, int64_t* size, int64_t* mtime_ms,
                       int64_t* kind) {
    char* text;
    struct stat info;
    int status;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    status = stat(text, &info);
    free(text);
    if (status != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    if (size != NULL) {
        *size = (int64_t)info.st_size;
    }
    if (mtime_ms != NULL) {
        *mtime_ms = (int64_t)LS_RT_IO_MTIME(info).tv_sec * 1000 +
                    (int64_t)LS_RT_IO_MTIME(info).tv_nsec / 1000000;
    }
    if (kind != NULL) {
        if (S_ISDIR(info.st_mode)) {
            *kind = 1;
        } else if (S_ISREG(info.st_mode)) {
            *kind = 0;
        } else {
            *kind = 2;
        }
    }
    return 0;
}

int64_t ls_io_remove_file(const LsString* path) {
    char* text;
    int status;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    status = unlink(text);
    free(text);
    if (status != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

int64_t ls_io_remove_dir(const LsString* path) {
    char* text;
    int status;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    status = rmdir(text);
    free(text);
    if (status != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

static int64_t ls_rt_io_remove_leaf_posix(const char* path,
                                          const struct stat* info) {
    int status;

    if (S_ISDIR(info->st_mode) && !S_ISLNK(info->st_mode)) {
        status = rmdir(path);
    } else {
        status = unlink(path);
    }
    if (status != 0 && errno != ENOENT) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

static int64_t ls_rt_io_remove_dir_tree_posix(const char* root) {
    LsRtIoNameList stack = {NULL, 0, 0};
    size_t root_length = strlen(root);
    char* root_copy = (char*)malloc(root_length + 1);

    if (root_copy == NULL) {
        ls_rt_alloc_failure(root_length + 1);
    }
    memcpy(root_copy, root, root_length + 1);
    ls_rt_io_name_list_push(&stack, root_copy);
    while (stack.count > 0) {
        char* current;
        struct stat info;

        stack.count -= 1;
        current = stack.items[stack.count];
        stack.items[stack.count] = NULL;
        if (lstat(current, &info) != 0) {
            if (errno == ENOENT) {
                free(current);
                continue;
            }
            free(current);
            ls_rt_io_name_list_free(&stack);
            return ls_rt_io_fail_errno(errno);
        }
        if (S_ISDIR(info.st_mode) && !S_ISLNK(info.st_mode)) {
            size_t current_length = strlen(current);
            char* again = (char*)malloc(current_length + 1);
            DIR* directory;
            int saved_errno = 0;
            bool has_children = false;

            if (again == NULL) {
                free(current);
                ls_rt_io_name_list_free(&stack);
                ls_rt_alloc_failure(current_length + 1);
            }
            memcpy(again, current, current_length + 1);
            directory = opendir(current);
            if (directory == NULL) {
                free(again);
                free(current);
                ls_rt_io_name_list_free(&stack);
                return ls_rt_io_fail_errno(errno);
            }
            ls_rt_io_name_list_push(&stack, again);
            for (;;) {
                struct dirent* entry;

                errno = 0;
                entry = readdir(directory);
                if (entry == NULL) {
                    saved_errno = errno;
                    break;
                }
                if (strcmp(entry->d_name, ".") == 0 ||
                    strcmp(entry->d_name, "..") == 0) {
                    continue;
                }
                {
                    size_t name_length = strlen(entry->d_name);
                    char* child =
                        (char*)malloc(current_length + name_length + 2);

                    if (child == NULL) {
                        (void)closedir(directory);
                        free(current);
                        ls_rt_io_name_list_free(&stack);
                        ls_rt_alloc_failure(0);
                    }
                    memcpy(child, current, current_length);
                    child[current_length] = '/';
                    memcpy(child + current_length + 1, entry->d_name,
                           name_length + 1);
                    ls_rt_io_name_list_push(&stack, child);
                    has_children = true;
                }
            }
            (void)closedir(directory);
            if (saved_errno != 0) {
                free(current);
                ls_rt_io_name_list_free(&stack);
                return ls_rt_io_fail_errno(saved_errno);
            }
            if (!has_children) {
                /* The directory is empty: discard its "again" entry (the last
                 * stack slot) and remove the directory as a leaf. */
                int64_t status;

                stack.count -= 1;
                free(stack.items[stack.count]);
                stack.items[stack.count] = NULL;
                status = ls_rt_io_remove_leaf_posix(current, &info);
                free(current);
                if (status != 0) {
                    ls_rt_io_name_list_free(&stack);
                    return -1;
                }
                continue;
            }
            free(current);
            continue;
        }
        {
            int64_t status = ls_rt_io_remove_leaf_posix(current, &info);

            free(current);
            if (status != 0) {
                ls_rt_io_name_list_free(&stack);
                return -1;
            }
        }
    }
    ls_rt_io_name_list_free(&stack);
    return 0;
}

int64_t ls_io_remove_dir_all(const LsString* path) {
    char* text;
    struct stat info;
    int status;
    int64_t result;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    status = stat(text, &info);
    if (status != 0) {
        result = ls_rt_io_fail_errno(errno);
        free(text);
        return result;
    }
    if (!S_ISDIR(info.st_mode)) {
        free(text);
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the path is not a directory");
    }
    result = ls_rt_io_remove_dir_tree_posix(text);
    free(text);
    return result;
}

static int64_t ls_rt_io_create_directory_tolerant(const char* path) {
    if (mkdir(path, 0777) == 0) {
        return 0;
    }
    if (errno == EEXIST) {
        struct stat info;

        if (stat(path, &info) == 0 && S_ISDIR(info.st_mode)) {
            return 0;
        }
    }
    return ls_rt_io_fail_errno(errno);
}

int64_t ls_io_create_dir(const LsString* path) {
    char* text;
    int status;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    status = mkdir(text, 0777);
    free(text);
    if (status != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

int64_t ls_io_create_dir_all(const LsString* path) {
    char* text;
    size_t index;
    size_t length;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    length = strlen(text);
    if (length == 0) {
        free(text);
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT, "the path is empty");
    }
    for (index = 1; index < length; index += 1) {
        char saved;

        if (text[index] != '/') {
            continue;
        }
        if (text[index - 1] == '/') {
            continue;
        }
        saved = text[index];
        text[index] = '\0';
        if (ls_rt_io_create_directory_tolerant(text) != 0) {
            text[index] = saved;
            free(text);
            return -1;
        }
        text[index] = saved;
    }
    {
        int64_t status = ls_rt_io_create_directory_tolerant(text);

        free(text);
        return status;
    }
}

int64_t ls_io_rename(const LsString* from, const LsString* to) {
    char* from_text;
    char* to_text;
    int status;

    ls_rt_io_error_reset();
    from_text = ls_rt_io_text_copy(from, "path");
    if (from_text == NULL) {
        return -1;
    }
    to_text = ls_rt_io_text_copy(to, "path");
    if (to_text == NULL) {
        free(from_text);
        return -1;
    }
    status = rename(from_text, to_text);
    free(from_text);
    free(to_text);
    if (status != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

LsString* ls_io_canonicalize(const LsString* path) {
    char* text;
    char* resolved;
    LsString* result;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return NULL;
    }
    resolved = realpath(text, NULL);
    free(text);
    if (resolved == NULL) {
        (void)ls_rt_io_fail_errno(errno);
        return NULL;
    }
    result = ls_rt_io_string_from_path_bytes(resolved);
    free(resolved);
    return result;
}

LsString* ls_io_current_dir(void) {
    size_t capacity = 256;
    int attempts;

    ls_rt_io_error_reset();
    for (attempts = 0; attempts < 12; attempts += 1) {
        char* buffer = (char*)malloc(capacity);

        if (buffer == NULL) {
            ls_rt_alloc_failure(capacity);
        }
        if (getcwd(buffer, capacity) != NULL) {
            LsString* result = ls_rt_io_string_from_path_bytes(buffer);

            free(buffer);
            return result;
        }
        {
            int code = errno;

            free(buffer);
            if (code != ERANGE) {
                (void)ls_rt_io_fail_errno(code);
                return NULL;
            }
        }
        if (capacity > ((size_t)1 << 20)) {
            break;
        }
        capacity *= 2;
    }
    (void)ls_rt_io_fail(LS_IO_ERROR_OTHER,
                        "the current directory path is too long");
    return NULL;
}

int64_t ls_io_set_current_dir(const LsString* path) {
    char* text;
    int status;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(path, "path");
    if (text == NULL) {
        return -1;
    }
    status = chdir(text);
    free(text);
    if (status != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

LsString* ls_io_temp_dir(void) {
    const char* value = getenv("TMPDIR");

    ls_rt_io_error_reset();
    if (value == NULL || value[0] == '\0') {
        value = "/tmp";
    }
    return ls_rt_io_string_from_path_bytes(value);
}

/* --- POSIX TCP sockets --- */

int64_t ls_net_listen(const LsString* address) {
    struct addrinfo* results = NULL;
    struct addrinfo* candidate;
    int listener = -1;
    int last_error = 0;

    ls_rt_io_error_reset();
    if (ls_rt_io_resolve_address(address, true, &results) != 0) {
        return -1;
    }
    for (candidate = results; candidate != NULL; candidate = candidate->ai_next) {
        int descriptor = socket(candidate->ai_family, candidate->ai_socktype,
                                candidate->ai_protocol);
        int reuse = 1;

        if (descriptor < 0) {
            last_error = errno;
            continue;
        }
        (void)setsockopt(descriptor, SOL_SOCKET, SO_REUSEADDR, &reuse,
                         (socklen_t)sizeof reuse);
        if (bind(descriptor, candidate->ai_addr,
                 (socklen_t)candidate->ai_addrlen) == 0 &&
            listen(descriptor, SOMAXCONN) == 0) {
            listener = descriptor;
            break;
        }
        last_error = errno;
        (void)close(descriptor);
    }
    freeaddrinfo(results);
    if (listener < 0) {
        if (last_error == 0) {
            return ls_rt_io_fail(LS_IO_ERROR_ADDRESS_NOT_AVAILABLE,
                                 "the address could not be bound");
        }
        return ls_rt_io_fail_errno(last_error);
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_SOCKET, (intptr_t)listener);
}

int64_t ls_net_local_port(int64_t handle) {
    LsRtIoHandle slot;
    struct sockaddr_storage address;
    socklen_t length = (socklen_t)sizeof address;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    if (getsockname((int)slot.native, (struct sockaddr*)&address, &length) != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    if (address.ss_family == AF_INET) {
        return (int64_t)ntohs(((struct sockaddr_in*)&address)->sin_port);
    }
    if (address.ss_family == AF_INET6) {
        return (int64_t)ntohs(((struct sockaddr_in6*)&address)->sin6_port);
    }
    return ls_rt_io_fail(LS_IO_ERROR_OTHER,
                         "the socket has an unsupported address family");
}

int64_t ls_net_accept(int64_t listener) {
    LsRtIoHandle slot;
    int connection;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(listener, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    for (;;) {
        connection = accept((int)slot.native, NULL, NULL);
        if (connection >= 0) {
            break;
        }
        if (errno == EINTR) {
            continue;
        }
        if (errno == EAGAIN || errno == EWOULDBLOCK) {
            return ls_rt_io_fail(LS_IO_ERROR_WOULD_BLOCK,
                                 "the operation would block");
        }
        return ls_rt_io_fail_errno(errno);
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_SOCKET, (intptr_t)connection);
}

int64_t ls_net_connect(const LsString* address) {
    struct addrinfo* results = NULL;
    struct addrinfo* candidate;
    int connection = -1;
    int last_error = 0;

    ls_rt_io_error_reset();
    if (ls_rt_io_resolve_address(address, false, &results) != 0) {
        return -1;
    }
    for (candidate = results; candidate != NULL; candidate = candidate->ai_next) {
        int descriptor = socket(candidate->ai_family, candidate->ai_socktype,
                                candidate->ai_protocol);
        int status;

        if (descriptor < 0) {
            last_error = errno;
            continue;
        }
        do {
            status = connect(descriptor, candidate->ai_addr,
                             (socklen_t)candidate->ai_addrlen);
        } while (status != 0 && errno == EINTR);
        if (status == 0) {
            connection = descriptor;
            break;
        }
        last_error = errno;
        (void)close(descriptor);
    }
    freeaddrinfo(results);
    if (connection < 0) {
        if (last_error == 0) {
            return ls_rt_io_fail(LS_IO_ERROR_OTHER,
                                 "no address was available for the connection");
        }
        return ls_rt_io_fail_errno(last_error);
    }
    return ls_rt_io_handle_allocate(LS_RT_IO_KIND_SOCKET, (intptr_t)connection);
}

int64_t ls_net_set_timeout(int64_t handle, int64_t milliseconds) {
    LsRtIoHandle slot;
    struct timeval value;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    if (milliseconds < 0) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the timeout must not be negative");
    }
    value.tv_sec = (time_t)(milliseconds / 1000);
    value.tv_usec = (suseconds_t)((milliseconds % 1000) * 1000);
    if (setsockopt((int)slot.native, SOL_SOCKET, SO_RCVTIMEO, &value,
                   (socklen_t)sizeof value) != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    if (setsockopt((int)slot.native, SOL_SOCKET, SO_SNDTIMEO, &value,
                   (socklen_t)sizeof value) != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

int64_t ls_net_shutdown(int64_t handle, int32_t how) {
    LsRtIoHandle slot;
    int mode;

    ls_rt_io_error_reset();
    if (!ls_rt_io_handle_acquire(handle, LS_RT_IO_ALLOW_SOCKET, &slot)) {
        return -1;
    }
    if (how == 0) {
        mode = SHUT_RD;
    } else if (how == 1) {
        mode = SHUT_WR;
    } else if (how == 2) {
        mode = SHUT_RDWR;
    } else {
        return ls_rt_io_fail(
            LS_IO_ERROR_INVALID_INPUT,
            "the shutdown mode must be 0 (read), 1 (write) or 2 (both)");
    }
    if (shutdown((int)slot.native, mode) != 0) {
        return ls_rt_io_fail_errno(errno);
    }
    return 0;
}

/* --- POSIX time --- */

int64_t ls_time_now_ms(void) {
    struct timespec value;

    if (clock_gettime(CLOCK_REALTIME, &value) != 0) {
        return 0;
    }
    return (int64_t)value.tv_sec * 1000 + (int64_t)value.tv_nsec / 1000000;
}

int64_t ls_time_monotonic_ns(void) {
    struct timespec value;

    if (clock_gettime(CLOCK_MONOTONIC, &value) != 0) {
        return 0;
    }
    return (int64_t)value.tv_sec * 1000000000LL + (int64_t)value.tv_nsec;
}

void ls_time_sleep_ms(int64_t milliseconds) {
    struct timespec request;

    if (milliseconds <= 0) {
        return;
    }
    request.tv_sec = (time_t)(milliseconds / 1000);
    request.tv_nsec = (long)((milliseconds % 1000) * 1000000);
    while (nanosleep(&request, &request) != 0) {
        if (errno != EINTR) {
            break;
        }
    }
}

static int64_t ls_rt_io_time_parts(struct tm* values, int32_t* year,
                                   int32_t* month, int32_t* day, int32_t* hour,
                                   int32_t* minute, int32_t* second,
                                   int32_t* weekday, int32_t* yearday) {
    if (values == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the timestamp could not be converted");
    }
    if (year != NULL) *year = values->tm_year + 1900;
    if (month != NULL) *month = values->tm_mon + 1;
    if (day != NULL) *day = values->tm_mday;
    if (hour != NULL) *hour = values->tm_hour;
    if (minute != NULL) *minute = values->tm_min;
    if (second != NULL) *second = values->tm_sec;
    if (weekday != NULL) *weekday = values->tm_wday;
    if (yearday != NULL) *yearday = values->tm_yday;
    return 0;
}

static int64_t ls_rt_io_seconds_from_ms(int64_t unix_ms, time_t* out) {
    int64_t seconds = unix_ms / 1000;

    if (unix_ms < 0 && unix_ms % 1000 != 0) {
        seconds -= 1;
    }
    *out = (time_t)seconds;
    if ((int64_t)*out != seconds) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the timestamp is out of range");
    }
    return 0;
}

int64_t ls_time_local_parts(int64_t unix_ms, int32_t* year, int32_t* month,
                            int32_t* day, int32_t* hour, int32_t* minute,
                            int32_t* second, int32_t* weekday,
                            int32_t* yearday) {
    time_t value;
    struct tm parts;

    ls_rt_io_error_reset();
    if (ls_rt_io_seconds_from_ms(unix_ms, &value) != 0) {
        return -1;
    }
    if (localtime_r(&value, &parts) == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the local time could not be computed");
    }
    return ls_rt_io_time_parts(&parts, year, month, day, hour, minute, second,
                               weekday, yearday);
}

int64_t ls_time_utc_parts(int64_t unix_ms, int32_t* year, int32_t* month,
                          int32_t* day, int32_t* hour, int32_t* minute,
                          int32_t* second, int32_t* weekday,
                          int32_t* yearday) {
    time_t value;
    struct tm parts;

    ls_rt_io_error_reset();
    if (ls_rt_io_seconds_from_ms(unix_ms, &value) != 0) {
        return -1;
    }
    if (gmtime_r(&value, &parts) == NULL) {
        return ls_rt_io_fail(LS_IO_ERROR_INVALID_INPUT,
                             "the UTC time could not be computed");
    }
    return ls_rt_io_time_parts(&parts, year, month, day, hour, minute, second,
                               weekday, yearday);
}

/* --- POSIX environment --- */

LsString* ls_env_var(const LsString* name) {
    char* text;
    const char* value;

    ls_rt_io_error_reset();
    text = ls_rt_io_text_copy(name, "name");
    if (text == NULL) {
        return NULL;
    }
    value = getenv(text);
    free(text);
    if (value == NULL) {
        return NULL;
    }
    return ls_rt_io_string_lossy(value, strlen(value));
}

LsVec_String* ls_env_vars(void) {
    extern char** environ;
    LsVec_String* result;
    char** entry;

    ls_rt_io_error_reset();
    result = ls_rt_io_vec_string_new();
    for (entry = environ; entry != NULL && *entry != NULL; entry += 1) {
        LsString* item;

        if ((*entry)[0] == '=') {
            continue;
        }
        item = ls_rt_io_string_lossy(*entry, strlen(*entry));
        if (item == NULL) {
            ls_release(result);
            return NULL;
        }
        ls_vec_string_push_owned(result, item);
    }
    if (result->len > 1) {
        qsort(result->data, result->len, sizeof *result->data,
              ls_rt_io_string_compare);
    }
    return result;
}

/* --- Platform lifetime (POSIX) --- */

static int ls_rt_io_platform_init(void) {
    return 0;
}

static void ls_rt_io_platform_shutdown(void) {
}

#endif /* platform implementation */

/* --- Handle-less public helpers --- */

int64_t ls_io_stdin(void) {
    return (int64_t)0;
}

int64_t ls_io_stdout(void) {
    return (int64_t)1;
}

int64_t ls_io_stderr(void) {
    return (int64_t)2;
}

int64_t ls_io_last_error_kind(void) {
    return ls_rt_io_error_kind;
}

LsString* ls_io_last_error_message(void) {
    size_t length = strlen(ls_rt_io_error_message);

    return ls_rt_io_string_lossy(ls_rt_io_error_message, length);
}

LsVec_String* ls_proc_args(void) {
    LsVec_String* result = ls_rt_io_vec_string_new();

    *result = ls_args();
    return result;
}

/* --- Runtime lifetime hooks, called by the core runtime --- */

void ls_rt_io_runtime_init(void) {
    if (ls_rt_io_initialized) {
        return;
    }
    ls_rt_io_table_create();
    (void)ls_rt_io_platform_init();
    ls_rt_io_initialized = 1;
}

void ls_rt_io_runtime_shutdown(void) {
    size_t index;

    if (!ls_rt_io_initialized) {
        return;
    }
    (void)fflush(NULL);
    ls_rt_io_lock();
    for (index = 0; index < ls_rt_io_handle_capacity; index += 1) {
        LsRtIoHandle* slot = &ls_rt_io_handles[index];

        if (slot->kind == LS_RT_IO_KIND_FILE ||
            slot->kind == LS_RT_IO_KIND_SOCKET) {
            ls_rt_io_native_close(slot->native, slot->kind);
            slot->native = 0;
            slot->kind = LS_RT_IO_KIND_FREE;
            slot->flags = 0;
            slot->next_free = LS_RT_IO_NO_SLOT;
        }
    }
    free(ls_rt_io_handles);
    ls_rt_io_handles = NULL;
    ls_rt_io_handle_capacity = 0;
    ls_rt_io_free_head = LS_RT_IO_NO_SLOT;
    ls_rt_io_initialized = 0;
    ls_rt_io_unlock();
    ls_rt_io_platform_shutdown();
}
