/* lsaca C17 runtime: String translation unit.
 *
 * Reference counted LsString objects, copy-on-write mutation, search and
 * transformation, split and parsing. The allocator/collector and the shared
 * private contract live in lsaca_rt.c / lsaca_rt_internal.h.
 */

#include "lsaca_rt_internal.h"

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

