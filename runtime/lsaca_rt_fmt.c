/* lsaca C17 runtime: formatting and output translation unit.
 *
 * Format builder primitives, shortest-round-trip float formatting, precision /
 * width / sign handling, Debug escaping and the print/println helpers.
 *
 * This is the only translation unit that links against <math.h> formatting
 * helpers (strtod/strtof/snprintf); the public ABI is in lsaca_rt.h.
 */

#include "lsaca_rt_internal.h"

/* ====================================================================== */
/* Formatting: builder primitives                                         */
/* ====================================================================== */

LsString* ls_format_begin(size_t capacity_hint) {
    return ls_str_alloc_capacity(capacity_hint);
}

/* Format builder appends land in the inlined fast path of the string unit:
 * when the accumulator is unique and has room this is a memcpy plus two size
 * updates, with no cross-translation-unit call and no temporary string. */
static LsString* ls_fmt_append(LsString* acc, const char* bytes, size_t len) {
    return ls_str_append_fast(acc, bytes, len);
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

/* Decimal conversion without printf: digits are written back to front into a
 * fixed buffer, so the cost per value is a handful of divisions and one small
 * memcpy into the accumulator. */
LsString* ls_fmt_u64(LsString* acc, uint64_t v) {
    char buffer[20];
    char* const end = buffer + sizeof buffer;
    char* start = end;

    do {
        *--start = (char)('0' + (int)(v % 10U));
        v /= 10U;
    } while (v != 0);
    return ls_fmt_append(acc, start, (size_t)(end - start));
}

LsString* ls_fmt_i64(LsString* acc, int64_t v) {
    char buffer[21];
    char* const end = buffer + sizeof buffer;
    char* start = end;
    uint64_t magnitude;
    bool negative = v < 0;

    /* Negating INT64_MIN is not representable; the unsigned subtraction gives
     * the correct magnitude 2^63 for it. */
    magnitude = negative ? (uint64_t)0 - (uint64_t)v : (uint64_t)v;
    do {
        *--start = (char)('0' + (int)(magnitude % 10U));
        magnitude /= 10U;
    } while (magnitude != 0);
    if (negative) {
        *--start = '-';
    }
    return ls_fmt_append(acc, start, (size_t)(end - start));
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

/* ====================================================================== */
/* Formatting: precision, sign and padding                                */
/* ====================================================================== */

static size_t ls_rt_encode_codepoint(uint32_t codepoint, char out[4]) {
    if (codepoint <= 0x7FU) {
        out[0] = (char)codepoint;
        return 1;
    }
    if (codepoint <= 0x7FFU) {
        out[0] = (char)(0xC0U | (codepoint >> 6));
        out[1] = (char)(0x80U | (codepoint & 0x3FU));
        return 2;
    }
    if (codepoint <= 0xFFFFU) {
        out[0] = (char)(0xE0U | (codepoint >> 12));
        out[1] = (char)(0x80U | ((codepoint >> 6) & 0x3FU));
        out[2] = (char)(0x80U | (codepoint & 0x3FU));
        return 3;
    }
    out[0] = (char)(0xF0U | (codepoint >> 18));
    out[1] = (char)(0x80U | ((codepoint >> 12) & 0x3FU));
    out[2] = (char)(0x80U | ((codepoint >> 6) & 0x3FU));
    out[3] = (char)(0x80U | (codepoint & 0x3FU));
    return 4;
}

/* Number of Unicode scalar values in a String, counted by UTF-8 lead bytes.
 * Malformed UTF-8 cannot be built by the language itself, so a byte count is
 * an adequate fallback here. */
static size_t ls_rt_codepoint_count(const LsString* s) {
    size_t i;
    size_t count = 0;

    for (i = 0; i < s->len; i += 1) {
        if (((unsigned char)s->data[i] & 0xC0U) != 0x80U) {
            count += 1;
        }
    }
    return count;
}

LsString* ls_fmt_i64_signed(LsString* acc, int64_t v, bool plus) {
    if (plus && v >= 0) {
        acc = ls_fmt_append(acc, "+", 1);
    }
    return ls_fmt_i64(acc, v);
}

LsString* ls_fmt_u64_signed(LsString* acc, uint64_t v, bool plus) {
    if (plus) {
        acc = ls_fmt_append(acc, "+", 1);
    }
    return ls_fmt_u64(acc, v);
}

static LsString* ls_rt_append_float_special(LsString* acc, double v,
                                            bool plus) {
    bool negative = v < 0;

    if (isnan(v)) {
        if (plus) {
            acc = ls_fmt_append(acc, "+", 1);
        }
        return ls_fmt_append(acc, "NaN", 3);
    }
    if (negative) {
        return ls_fmt_append(acc, "-inf", 4);
    }
    if (plus) {
        acc = ls_fmt_append(acc, "+", 1);
    }
    return ls_fmt_append(acc, "inf", 3);
}

/* Fixed-notation float formatting with "precision" digits after the decimal
 * point. printf("%.*f") gives the exact rounding we need; the buffer is sized
 * from the maximum decimal exponent of a double (308 digits before the point)
 * plus the requested precision. */
static LsString* ls_rt_format_float_prec(LsString* acc, double v,
                                        int32_t precision, bool plus) {
    size_t capacity;
    char* buf;
    int written;

    if (isnan(v) || isinf(v)) {
        return ls_rt_append_float_special(acc, v, plus);
    }
    if (plus && !(v < 0) && signbit(v) == 0) {
        acc = ls_fmt_append(acc, "+", 1);
    }
    if (precision < 0) {
        return ls_rt_format_float(acc, false, v, false);
    }
    if ((uint64_t)precision > 10000U) {
        precision = 10000;
    }
    capacity = 340U + (size_t)precision;
    buf = (char*)malloc(capacity);
    if (buf == NULL) {
        ls_rt_alloc_failure(capacity);
    }
    written = snprintf(buf, capacity, "%.*f", (int)precision, v);
    if (written > 0) {
        acc = ls_fmt_append(acc, buf, (size_t)written);
    }
    free(buf);
    return acc;
}

LsString* ls_fmt_f64_prec(LsString* acc, double v, int32_t precision,
                          bool plus) {
    return ls_rt_format_float_prec(acc, v, precision, plus);
}

LsString* ls_fmt_f32_prec(LsString* acc, float v, int32_t precision,
                          bool plus) {
    return ls_rt_format_float_prec(acc, (double)v, precision, plus);
}

LsString* ls_fmt_str_prec(LsString* acc, const LsString* v, int32_t precision) {
    size_t i = 0;
    size_t kept = 0;

    if (precision < 0 || (size_t)precision >= ls_rt_codepoint_count(v)) {
        return ls_format_push_str(acc, v);
    }
    while (i < v->len && kept < (size_t)precision) {
        i += 1;
        while (i < v->len && ((unsigned char)v->data[i] & 0xC0U) == 0x80U) {
            i += 1;
        }
        kept += 1;
    }
    return ls_fmt_append(acc, v->data, i);
}

static LsString* ls_rt_pad_zero(LsString* acc, const LsString* piece,
                                size_t padding) {
    size_t i;

    if (piece->len > 0 && (piece->data[0] == '+' || piece->data[0] == '-')) {
        acc = ls_fmt_append(acc, piece->data, 1);
        for (i = 0; i < padding; i += 1) {
            acc = ls_fmt_append(acc, "0", 1);
        }
        return ls_fmt_append(acc, piece->data + 1, piece->len - 1);
    }
    for (i = 0; i < padding; i += 1) {
        acc = ls_fmt_append(acc, "0", 1);
    }
    return ls_format_push_str(acc, piece);
}

LsString* ls_fmt_apply_width(LsString* acc, LsString* piece, int64_t width,
                             int32_t align, uint32_t fill, bool zero_pad) {
    size_t count;
    size_t padding;
    size_t left = 0;
    size_t right = 0;
    size_t i;
    char fill_bytes[4];
    size_t fill_len;

    if (width <= 1) {
        return ls_format_push_str(acc, piece);
    }
    count = ls_rt_codepoint_count(piece);
    if ((size_t)width <= count) {
        return ls_format_push_str(acc, piece);
    }
    padding = (size_t)width - count;
    if (zero_pad) {
        return ls_rt_pad_zero(acc, piece, padding);
    }
    if (align == LS_ALIGN_RIGHT) {
        left = padding;
    } else if (align == LS_ALIGN_CENTER) {
        left = padding / 2;
        right = padding - left;
    } else {
        right = padding;
    }
    fill_len = ls_rt_encode_codepoint(fill, fill_bytes);
    for (i = 0; i < left; i += 1) {
        acc = ls_fmt_append(acc, fill_bytes, fill_len);
    }
    acc = ls_format_push_str(acc, piece);
    for (i = 0; i < right; i += 1) {
        acc = ls_fmt_append(acc, fill_bytes, fill_len);
    }
    return acc;
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

