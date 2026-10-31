/* lsaca C17 runtime: String translation unit.
 *
 * Reference counted LsString objects, copy-on-write mutation, search and
 * transformation, split and parsing. The allocator/collector and the shared
 * private contract live in lsaca_rt.c / lsaca_rt_internal.h.
 */

#include "lsaca_rt_internal.h"
#include "lsaca_rt_unicode.h"

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

LsString* ls_str_alloc_capacity(size_t capacity) {
    LsString* s;
    size_t payload_size;

    if (capacity > SIZE_MAX - sizeof(LsString) - 1) {
        ls_rt_alloc_failure(capacity);
    }
    payload_size = sizeof(LsString) + capacity + 1;
    s = (LsString*)ls_rt_alloc_payload(payload_size, NULL, NULL);
    s->len = 0;
    s->cap = capacity;
    s->data = (char*)((char*)s + sizeof(LsString));
    s->data[0] = '\0';
    return s;
}

LsString* ls_str_alloc_from(const char* bytes, size_t len) {
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
    /* A shared receiver has to detach (copy on write) and a unique one has to
     * grow when the buffer is full; both are the same single allocation. */
    if (ls_rt_refcount_of(s) > 1 || needed > s->cap) {
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

/* Slow path: makes room (growing or detaching exactly once) and copies the
 * bytes. */
LsString* ls_str_append_slow(LsString* s, const char* bytes, size_t count) {
    s = ls_str_ensure_room(s, count);
    if (count != 0 && bytes != NULL) {
        memcpy(s->data + s->len, bytes, count);
    }
    s->len += count;
    s->data[s->len] = '\0';
    return s;
}

/* Out-of-line entry point: callers that cannot see the internal inline fast
 * path (other translation units) land here; it immediately tries the same
 * fast path before falling back to the slow one. */
LsString* ls_str_append_bytes(LsString* s, const char* bytes, size_t count) {
    return ls_str_append_fast(s, bytes, count);
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

/* Immortal interned string literal. The LsString header itself is allocated
 * from the immortal allocator (never destroyed, not live-counted or reported
 * by the debug leak report); the byte buffer is a separate malloc'd array that
 * is intentionally never freed, so the string can be used for the whole
 * process lifetime. Embedded NUL bytes are copied exactly. Generated code
 * calls this once per distinct literal and retains the cached handle per use,
 * so ls_release() only drops the reference count. */
LsString* ls_str_static(const char* data, size_t len) {
    LsString* s = (LsString*)ls_rt_alloc_immortal(sizeof(LsString));
    char* bytes;
    if (data == NULL) {
        len = 0;
    }
    bytes = (char*)malloc(len + 1);
    if (bytes == NULL) {
        ls_rt_alloc_failure(len + 1);
    }
    if (len != 0) {
        memcpy(bytes, data, len);
    }
    bytes[len] = '\0';
    s->len = len;
    s->cap = len;
    s->data = bytes;
    return s;
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
    return ls_str_append_fast(s, suffix->data, extra);
}

LsString* ls_str_push_cstr(LsString* s, const char* suffix) {
    if (suffix == NULL) {
        return s;
    }
    return ls_str_append_fast(s, suffix, strlen(suffix));
}

LsString* ls_str_push_char(LsString* s, char c) {
    return ls_str_push_byte_fast(s, c);
}

LsString* ls_str_clear(LsString* s) {
    if (ls_rt_refcount_of(s) > 1) {
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
        /* The empty pattern matches at every Unicode scalar boundary, not at
         * every byte boundary (the buffer is always valid UTF-8). */
        count = (size_t)ls_str_char_count(s) + 1;
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
        in = 0;
        while (in < s->len) {
            size_t length = ls_rt_utf8_decode_length(
                (const unsigned char*)s->data + in, s->len - in);
            if (length == 0) {
                length = 1; /* defensive: buffers are always valid UTF-8 */
            }
            memcpy(result->data + out, s->data + in, length);
            out += length;
            if (to->len != 0) {
                memcpy(result->data + out, to->data, to->len);
            }
            out += to->len;
            in += length;
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

/* Apply the generated Unicode default case mapping scalar by scalar. One
 * scalar can expand to several (for example sharp s uppercases to "SS" and
 * U+0130 lowercases to "i" + U+0307), so the result is built by appending
 * encoded scalars instead of rewriting bytes in place. The output buffer stays
 * valid UTF-8 by construction. */
static LsString* ls_str_map_case(const LsString* s, bool upper) {
    LsString* result = ls_str_alloc_capacity(s->len);
    size_t in = 0;

    while (in < s->len) {
        const unsigned char* bytes = (const unsigned char*)s->data + in;
        size_t length = ls_rt_utf8_decode_length(bytes, s->len - in);
        uint32_t mapped[LS_RT_UNICODE_MAX_EXPANSION];
        size_t count;
        size_t i;

        if (length == 0) {
            length = 1; /* defensive: buffers are always valid UTF-8 */
        }
        if (upper) {
            count = ls_rt_unicode_upper(ls_rt_utf8_scalar_at(bytes), mapped);
        } else {
            count = ls_rt_unicode_lower(ls_rt_utf8_scalar_at(bytes), mapped);
        }
        for (i = 0; i < count; i += 1) {
            char encoded[4];
            size_t encoded_len = ls_rt_utf8_encode_scalar(mapped[i], encoded);
            result = ls_str_ensure_room(result, encoded_len);
            memcpy(result->data + result->len, encoded, encoded_len);
            result->len += encoded_len;
            result->data[result->len] = '\0';
        }
        in += length;
    }
    return result;
}

LsString* ls_str_to_uppercase(const LsString* s) {
    return ls_str_map_case(s, true);
}

LsString* ls_str_to_lowercase(const LsString* s) {
    return ls_str_map_case(s, false);
}

LsString* ls_str_trim(const LsString* s) {
    size_t start = 0;
    size_t end = s->len;

    while (start < end) {
        size_t length = ls_rt_utf8_decode_length(
            (const unsigned char*)s->data + start, end - start);
        if (length == 0) {
            break; /* defensive: buffers are always valid UTF-8 */
        }
        if (!ls_rt_unicode_is_whitespace(ls_rt_utf8_scalar_at(
                (const unsigned char*)s->data + start))) {
            break;
        }
        start += length;
    }
    while (end > start) {
        size_t back = 1;
        size_t length;

        while (end - back > start &&
               ((unsigned char)s->data[end - back] & 0xC0u) == 0x80u) {
            back += 1;
        }
        length = ls_rt_utf8_decode_length(
            (const unsigned char*)s->data + (end - back), back);
        if (length == 0 || length != back) {
            break; /* defensive: buffers are always valid UTF-8 */
        }
        if (!ls_rt_unicode_is_whitespace(ls_rt_utf8_scalar_at(
                (const unsigned char*)s->data + (end - back)))) {
            break;
        }
        end -= back;
    }
    return ls_str_alloc_from(s->data + start, end - start);
}

/* Panic messages for str slicing follow Rust: the offending byte index is
 * named, and the string is rendered backtick-quoted and truncated to 256 bytes
 * with a trailing "...". */

/* Render the contents of s for the backtick-quoted slice representation used
 * by Rust's str slicing panics: escape backslash, backtick and the common
 * control characters, truncate to 256 bytes and append "...". */
static size_t ls_str_slice_quote(char* out, size_t out_size,
                                 const LsString* s) {
    size_t limit = s->len < 256 ? s->len : 256;
    size_t outlen = 0;
    size_t i;

    for (i = 0; i < limit; i += 1) {
        unsigned char c = (unsigned char)s->data[i];
        char escaped[8];
        size_t escaped_len = 0;
        if (c == '\\' || c == 0x60) {
            escaped[0] = '\\';
            escaped[1] = (char)c;
            escaped_len = 2;
        } else if (c == '\n') {
            escaped[0] = '\\';
            escaped[1] = 'n';
            escaped_len = 2;
        } else if (c == '\r') {
            escaped[0] = '\\';
            escaped[1] = 'r';
            escaped_len = 2;
        } else if (c == '\t') {
            escaped[0] = '\\';
            escaped[1] = 't';
            escaped_len = 2;
        } else if (c < 0x20u || c == 0x7Fu) {
            int written =
                snprintf(escaped, sizeof escaped, "\\u{%x}", (unsigned)c);
            if (written > 0) {
                escaped_len = (size_t)written < sizeof escaped
                                  ? (size_t)written
                                  : sizeof escaped - 1;
            }
        } else {
            escaped[0] = (char)c;
            escaped_len = 1;
        }
        if (outlen + escaped_len + 1 > out_size) {
            break;
        }
        memcpy(out + outlen, escaped, escaped_len);
        outlen += escaped_len;
    }
    if (s->len > 256 && outlen + 4 <= out_size) {
        memcpy(out + outlen, "...", 3);
        outlen += 3;
    }
    out[outlen] = '\0';
    return outlen;
}

/* Render one scalar as the quote-delimited char literal used by Rust's
 * "not a char boundary" panic message (for example 'e' with an acute accent,
 * or '\n'). */
static size_t ls_str_slice_char_repr(char* out, size_t out_size,
                                     uint32_t scalar) {
    if (scalar == 0x27u) { /* apostrophe */
        out[0] = '\\';
        out[1] = '\'';
        out[2] = '\0';
        return 2;
    }
    if (scalar == 0x5Cu) { /* backslash */
        out[0] = '\\';
        out[1] = '\\';
        out[2] = '\0';
        return 2;
    }
    if (scalar == 0x09u) {
        out[0] = '\\';
        out[1] = 't';
        out[2] = '\0';
        return 2;
    }
    if (scalar == 0x0Au) {
        out[0] = '\\';
        out[1] = 'n';
        out[2] = '\0';
        return 2;
    }
    if (scalar == 0x0Du) {
        out[0] = '\\';
        out[1] = 'r';
        out[2] = '\0';
        return 2;
    }
    if (ls_rt_unicode_char_is_printable(scalar)) {
        /* ls_rt_utf8_encode_scalar does not NUL-terminate; the caller renders
         * this buffer with snprintf, so terminate it explicitly. */
        size_t length = ls_rt_utf8_encode_scalar(scalar, out);
        if (length < out_size) {
            out[length] = '\0';
        }
        return length;
    }
    {
        int written = snprintf(out, out_size, "\\u{%x}", (unsigned)scalar);
        if (written < 0) {
            out[0] = '\0';
            return 0;
        }
        return (size_t)written < out_size ? (size_t)written : out_size - 1;
    }
}

static void ls_str_panic_slice_bounds(const LsString* s, size_t index,
                                      const char* file, int32_t line,
                                      int32_t col) {
    char quoted[560];
    char message[640];

    ls_str_slice_quote(quoted, sizeof quoted, s);
    snprintf(message, sizeof message, "byte index %zu is out of bounds of \x60%s\x60",
             index, quoted);
    ls_panic_cstr(file, line, col, message);
}

static void ls_str_panic_slice_boundary(const LsString* s, size_t index,
                                        const char* file, int32_t line,
                                        int32_t col) {
    size_t byte_start = 0;
    size_t length = 0;
    size_t scan = 0;
    char quoted[560];
    char char_repr[16];
    char message[700];

    while (scan < s->len) {
        size_t current = ls_rt_utf8_decode_length(
            (const unsigned char*)s->data + scan, s->len - scan);
        if (current == 0) {
            current = 1; /* defensive: buffers are always valid UTF-8 */
        }
        if (index < scan + current) {
            byte_start = scan;
            length = current;
            break;
        }
        scan += current;
    }
    ls_str_slice_quote(quoted, sizeof quoted, s);
    if (s->len != 0) {
        ls_str_slice_char_repr(
            char_repr, sizeof char_repr,
            ls_rt_utf8_scalar_at((const unsigned char*)s->data + byte_start));
    } else {
        char_repr[0] = '\0';
    }
    snprintf(message, sizeof message,
             "byte index %zu is not a char boundary; it is inside '%s' "
             "(bytes %zu..%zu) of \x60%s\x60",
             index, char_repr, byte_start, byte_start + length, quoted);
    ls_panic_cstr(file, line, col, message);
}

LsString* ls_str_substring(const LsString* s, size_t start, size_t len,
                           const char* file, int32_t line, int32_t col) {
    size_t end;
    size_t reported;

    if (len > s->len || start > s->len - len) {
        reported = start > SIZE_MAX - len ? SIZE_MAX : start + len;
        ls_str_panic_slice_bounds(s, reported, file, line, col);
    }
    end = start + len;
    if (start != 0 && start != s->len &&
        ((unsigned char)s->data[start] & 0xC0u) == 0x80u) {
        ls_str_panic_slice_boundary(s, start, file, line, col);
    }
    if (end != 0 && end != s->len &&
        ((unsigned char)s->data[end] & 0xC0u) == 0x80u) {
        ls_str_panic_slice_boundary(s, end, file, line, col);
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
/* UTF-8 validation                                                       */
/* ====================================================================== */

/* Length of the well-formed UTF-8 sequence that starts at p (1, 2, 3 or 4
 * bytes), or 0 when the bytes are truncated or invalid. Strict by design:
 * overlong forms, UTF-16 surrogate halves and scalars above U+10FFFF are
 * rejected exactly like Rust's str::from_utf8. */
size_t ls_rt_utf8_decode_length(const unsigned char* p, size_t remaining) {
    unsigned char lead;
    size_t length;
    size_t i;

    if (remaining == 0) {
        return 0;
    }
    lead = p[0];
    if (lead < 0x80) {
        return 1;
    }
    if ((lead & 0xE0) == 0xC0) {
        length = 2;
        if (lead < 0xC2) {
            return 0; /* overlong two byte form (C0/C1) */
        }
    } else if ((lead & 0xF0) == 0xE0) {
        length = 3;
    } else if ((lead & 0xF8) == 0xF0) {
        length = 4;
        if (lead > 0xF4) {
            return 0; /* above U+10FFFF */
        }
    } else {
        return 0; /* continuation byte or invalid lead byte */
    }
    if (length > remaining || (p[1] & 0xC0) != 0x80) {
        return 0;
    }
    if (length == 3) {
        if (lead == 0xE0 && p[1] < 0xA0) {
            return 0; /* overlong three byte form */
        }
        if (lead == 0xED && p[1] >= 0xA0) {
            return 0; /* surrogate half */
        }
    } else if (length == 4) {
        if (lead == 0xF0 && p[1] < 0x90) {
            return 0; /* overlong four byte form */
        }
        if (lead == 0xF4 && p[1] > 0x8F) {
            return 0; /* above U+10FFFF */
        }
    }
    for (i = 2; i < length; i += 1) {
        if ((p[i] & 0xC0) != 0x80) {
            return 0;
        }
    }
    return length;
}

bool ls_rt_utf8_validate(const char* data, size_t len) {
    size_t offset = 0;

    if (len == 0) {
        return true;
    }
    if (data == NULL) {
        return false;
    }
    while (offset < len) {
        size_t length = ls_rt_utf8_decode_length(
            (const unsigned char*)data + offset, len - offset);
        if (length == 0) {
            return false;
        }
        offset += length;
    }
    return true;
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
/* Unicode scalar helpers                                                 */
/* ====================================================================== */

/* Encode one Unicode scalar value as UTF-8. Returns the number of bytes
 * written (1..4). Invalid scalars (UTF-16 surrogate halves and values above
 * U+10FFFF) are encoded as U+FFFD REPLACEMENT CHARACTER; the language's char
 * type only admits valid scalars, so this is a defensive fallback. */
size_t ls_rt_utf8_encode_scalar(uint32_t scalar, char out[4]) {
    if (scalar > 0x10FFFFu || (scalar >= 0xD800u && scalar <= 0xDFFFu)) {
        scalar = 0xFFFDu;
    }
    if (scalar < 0x80u) {
        out[0] = (char)scalar;
        return 1;
    }
    if (scalar < 0x800u) {
        out[0] = (char)(0xC0u | (scalar >> 6));
        out[1] = (char)(0x80u | (scalar & 0x3Fu));
        return 2;
    }
    if (scalar < 0x10000u) {
        out[0] = (char)(0xE0u | (scalar >> 12));
        out[1] = (char)(0x80u | ((scalar >> 6) & 0x3Fu));
        out[2] = (char)(0x80u | (scalar & 0x3Fu));
        return 3;
    }
    out[0] = (char)(0xF0u | (scalar >> 18));
    out[1] = (char)(0x80u | ((scalar >> 12) & 0x3Fu));
    out[2] = (char)(0x80u | ((scalar >> 6) & 0x3Fu));
    out[3] = (char)(0x80u | (scalar & 0x3Fu));
    return 4;
}

/* Scalar stored at p, which must start a well-formed UTF-8 sequence. */
uint32_t ls_rt_utf8_scalar_at(const unsigned char* p) {
    unsigned char lead = p[0];

    if (lead < 0x80u) {
        return (uint32_t)lead;
    }
    if ((lead & 0xE0u) == 0xC0u) {
        return ((uint32_t)(lead & 0x1Fu) << 6) | (uint32_t)(p[1] & 0x3Fu);
    }
    if ((lead & 0xF0u) == 0xE0u) {
        return ((uint32_t)(lead & 0x0Fu) << 12) |
               ((uint32_t)(p[1] & 0x3Fu) << 6) | (uint32_t)(p[2] & 0x3Fu);
    }
    return ((uint32_t)(lead & 0x07u) << 18) |
           ((uint32_t)(p[1] & 0x3Fu) << 12) |
           ((uint32_t)(p[2] & 0x3Fu) << 6) | (uint32_t)(p[3] & 0x3Fu);
}

int64_t ls_str_char_count(const LsString* s) {
    int64_t count = 0;
    size_t i = 0;

    while (i < s->len) {
        size_t length = ls_rt_utf8_decode_length(
            (const unsigned char*)s->data + i, s->len - i);
        if (length == 0) {
            break;
        }
        i += length;
        count += 1;
    }
    return count;
}

bool ls_str_char_at(const LsString* s, int64_t char_index, uint32_t* out) {
    int64_t index = 0;
    size_t i = 0;

    if (char_index < 0) {
        return false;
    }
    while (i < s->len) {
        size_t length = ls_rt_utf8_decode_length(
            (const unsigned char*)s->data + i, s->len - i);
        if (length == 0) {
            return false;
        }
        if (index == char_index) {
            *out = ls_rt_utf8_scalar_at((const unsigned char*)s->data + i);
            return true;
        }
        index += 1;
        i += length;
    }
    return false;
}

bool ls_str_is_char_boundary(const LsString* s, int64_t byte_index) {
    size_t index;

    if (byte_index < 0) {
        return false;
    }
    index = (size_t)byte_index;
    if (index > s->len) {
        return false;
    }
    if (index == 0 || index == s->len) {
        return true;
    }
    return ((unsigned char)s->data[index] & 0xC0u) != 0x80u;
}

LsString* ls_str_from_scalar(uint32_t scalar) {
    char bytes[4];
    size_t length = ls_rt_utf8_encode_scalar(scalar, bytes);
    return ls_str_alloc_from(bytes, length);
}

LsString* ls_str_push_scalar(LsString* s, uint32_t scalar) {
    char bytes[4];
    size_t length = ls_rt_utf8_encode_scalar(scalar, bytes);
    if (length == 0) {
        return s;
    }
    return ls_str_append_fast(s, bytes, length);
}

LsString* ls_str_pop_scalar(LsString* s, bool* out_some, uint32_t* out_scalar) {
    size_t start;

    if (s->len == 0) {
        *out_some = false;
        *out_scalar = 0;
        return s;
    }
    start = s->len - 1;
    while (start > 0 && ((unsigned char)s->data[start] & 0xC0u) == 0x80u) {
        start -= 1;
    }
    *out_scalar = ls_rt_utf8_scalar_at((const unsigned char*)s->data + start);
    *out_some = true;
    s = ls_str_ensure_room(s, 0);
    s->len = start;
    s->data[start] = '\0';
    return s;
}

LsVec_char ls_str_chars(const LsString* s) {
    LsVec_char vector;
    size_t i = 0;
    size_t index = 0;

    vector.len = (size_t)ls_str_char_count(s);
    vector.cap = vector.len;
    vector.data = (uint32_t*)ls_vec_alloc(vector.len, sizeof(uint32_t));
    while (i < s->len) {
        size_t length = ls_rt_utf8_decode_length(
            (const unsigned char*)s->data + i, s->len - i);
        if (length == 0) {
            break;
        }
        vector.data[index] = ls_rt_utf8_scalar_at((const unsigned char*)s->data + i);
        index += 1;
        i += length;
    }
    return vector;
}


