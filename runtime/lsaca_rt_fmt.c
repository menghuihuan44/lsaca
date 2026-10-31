/* lsaca C17 runtime: formatting and output translation unit.
 *
 * Format builder primitives, shortest-round-trip float formatting, precision /
 * width / sign handling, Debug escaping and the print/println helpers.
 *
 * This is the only translation unit that links against <math.h> formatting
 * helpers (strtod/strtof/snprintf); the public ABI is in lsaca_rt.h.
 */

#include "lsaca_rt_internal.h"
#include "lsaca_rt_unicode.h"

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

/* The shortest decimal representation is computed exactly with a big-integer
 * free-format digit generator (Burger-Dybvig / Dragon4). The value is written
 * as an integer rational R/S scaled by a power of ten; digits are emitted
 * until either truncating or rounding the remainder is guaranteed to parse
 * back to the same IEEE value, and the final digit is rounded to nearest with
 * ties to even. This is exact for every finite value, needs no tables, and
 * replaces the old brute-force snprintf/strtod loop (18.85 us per value). */

#define LS_RT_BIG_WORDS 36u

/* Fixed-width unsigned big integer, little-endian 64-bit limbs with a used
 * length. 36 limbs (2304 bits) cover the widest intermediate: the scaled
 * denominator of the largest double (~2^1996) and the scaled numerator of the
 * smallest subnormal printed with 17 significant digits (~2^1131). Only the
 * used limbs are touched, which keeps the common case (about a dozen limbs)
 * cheap. */
typedef struct {
    uint32_t length;
    uint64_t limb[LS_RT_BIG_WORDS];
} LsRtBig;

static void ls_rt_big_zero(LsRtBig* x) {
    x->length = 0;
}

static void ls_rt_big_trim(LsRtBig* x) {
    while (x->length > 0 && x->limb[x->length - 1] == 0) {
        x->length -= 1;
    }
}

static void ls_rt_big_from_u64(LsRtBig* x, uint64_t v) {
    ls_rt_big_zero(x);
    if (v != 0) {
        x->limb[0] = v;
        x->length = 1;
    }
}

static void ls_rt_big_copy(LsRtBig* dst, const LsRtBig* src) {
    uint32_t i;
    dst->length = src->length;
    for (i = 0; i < src->length; i += 1) {
        dst->limb[i] = src->limb[i];
    }
}

static void ls_rt_big_shl(LsRtBig* x, unsigned bits) {
    unsigned words = bits / 64u;
    unsigned rem = bits % 64u;
    uint32_t i;

    if (x->length == 0) {
        return;
    }
    if (words != 0) {
        for (i = x->length; i > 0; i -= 1) {
            x->limb[i - 1u + words] = x->limb[i - 1u];
        }
        for (i = 0; i < words; i += 1) {
            x->limb[i] = 0;
        }
        x->length += words;
    }
    if (rem != 0) {
        uint64_t carry = 0;
        for (i = 0; i < x->length; i += 1) {
            uint64_t word = x->limb[i];
            x->limb[i] = (word << rem) | carry;
            carry = word >> (64u - rem);
        }
        if (carry != 0 && x->length < LS_RT_BIG_WORDS) {
            x->limb[x->length] = carry;
            x->length += 1;
        }
    }
}

/* x /= 2, used for the half-width lower boundary of powers of two. */
static void ls_rt_big_shr1(LsRtBig* x) {
    uint32_t i;
    for (i = 0; i + 1 < x->length; i += 1) {
        x->limb[i] = (x->limb[i] >> 1) | (x->limb[i + 1] << 63);
    }
    if (x->length > 0) {
        x->limb[x->length - 1] >>= 1;
    }
    ls_rt_big_trim(x);
}

/* x *= factor; factor must fit in 32 bits. */
static void ls_rt_big_mul_small(LsRtBig* x, uint32_t factor) {
    uint64_t carry = 0;
    uint32_t i;

    for (i = 0; i < x->length; i += 1) {
        uint64_t word = x->limb[i];
        uint64_t low = (uint64_t)(uint32_t)word * factor;
        uint64_t high = (uint64_t)(uint32_t)(word >> 32) * factor;
        uint64_t middle = (low >> 32) + (uint64_t)(uint32_t)high;
        uint64_t result_low = (low & 0xFFFFFFFFu) | (middle << 32);
        uint64_t result_high = (high >> 32) + (middle >> 32);
        uint64_t sum = result_low + carry;
        carry = result_high + (sum < result_low ? 1u : 0u);
        x->limb[i] = sum;
    }
    if (carry != 0 && x->length < LS_RT_BIG_WORDS) {
        x->limb[x->length] = carry;
        x->length += 1;
    }
}

static void ls_rt_big_mul_pow10(LsRtBig* x, int power) {
    static const uint32_t pow10[10] = {1u,        10u,        100u,
                                       1000u,     10000u,     100000u,
                                       1000000u,  10000000u,  100000000u,
                                       1000000000u};
    while (power >= 9) {
        ls_rt_big_mul_small(x, 1000000000u);
        power -= 9;
    }
    if (power > 0) {
        ls_rt_big_mul_small(x, pow10[power]);
    }
}

static int ls_rt_big_cmp(const LsRtBig* a, const LsRtBig* b) {
    uint32_t i;
    if (a->length != b->length) {
        return a->length < b->length ? -1 : 1;
    }
    for (i = a->length; i > 0; i -= 1) {
        if (a->limb[i - 1u] != b->limb[i - 1u]) {
            return a->limb[i - 1u] < b->limb[i - 1u] ? -1 : 1;
        }
    }
    return 0;
}

/* a -= b; requires a >= b. */
static void ls_rt_big_sub(LsRtBig* a, const LsRtBig* b) {
    uint64_t borrow = 0;
    uint32_t i;
    for (i = 0; i < a->length; i += 1) {
        uint64_t low = i < b->length ? b->limb[i] : 0u;
        uint64_t current = a->limb[i];
        uint64_t difference = current - low - borrow;
        borrow = (current < low || (borrow != 0 && current == low)) ? 1u : 0u;
        a->limb[i] = difference;
    }
    ls_rt_big_trim(a);
}

/* Round the digit string up by one unit in its last position, propagating the
 * carry; an all-nine string becomes "1" with the decimal exponent bumped. */
static void ls_rt_digits_round_up(char* digits, size_t* out_ndigits,
                                  int* out_exponent) {
    size_t index = *out_ndigits;
    while (index > 0) {
        index -= 1;
        if (digits[index] != '9') {
            digits[index] = (char)(digits[index] + 1);
            *out_ndigits = index + 1;
            return;
        }
    }
    digits[0] = '1';
    *out_ndigits = 1;
    *out_exponent += 1;
}

/* Shortest round-trip significant digits for one finite positive IEEE value.
 * bits is the raw bit pattern with the sign bit cleared; is_f32 selects the
 * 24-bit/8-bit-exponent format. On return digits[0..*out_ndigits) holds the
 * significant digits and value = 0.digits * 10^(*out_exponent + 1). */
static void ls_rt_shortest_float(uint64_t bits, bool is_f32, char* digits,
                                 size_t* out_ndigits, int* out_exponent) {
    uint64_t fraction;
    uint64_t mantissa;
    int biased;
    int exponent;
    int scale;
    int max_digits;
    bool power_of_two;
    bool inclusive;
    int attempt;
    LsRtBig r;
    LsRtBig s;
    LsRtBig m_plus;
    LsRtBig m_minus;
    LsRtBig s2;
    LsRtBig s4;
    LsRtBig s8;
    size_t ndigits = 0;

    if (is_f32) {
        fraction = bits & 0x7FFFFFu;
        biased = (int)((bits >> 23) & 0xFFu);
        max_digits = 9;
        if (biased == 0) {
            mantissa = fraction;
            exponent = -149;
        } else {
            mantissa = fraction | 0x800000u;
            exponent = biased - 150;
        }
    } else {
        fraction = bits & 0xFFFFFFFFFFFFFULL;
        biased = (int)((bits >> 52) & 0x7FFu);
        max_digits = 17;
        if (biased == 0) {
            mantissa = fraction;
            exponent = -1074;
        } else {
            mantissa = fraction | 0x10000000000000ULL;
            exponent = biased - 1075;
        }
    }
    power_of_two = fraction == 0;
    /* A decimal exactly on the midpoint between two IEEE values parses back to
     * the value whose significand is even. */
    inclusive = (mantissa & 1u) == 0;

    {
        double magnitude;
        if (is_f32) {
            uint32_t raw = (uint32_t)bits & 0x7FFFFFFFu;
            float value32;
            memcpy(&value32, &raw, sizeof value32);
            magnitude = (double)value32;
        } else {
            double value64;
            memcpy(&value64, &bits, sizeof value64);
            magnitude = fabs(value64);
        }
        scale = (int)floor(log10(magnitude));
    }

    for (attempt = 0; attempt < 4; attempt += 1) {
        /* v = f * 2^e is scaled by 4 so that two extra decimal digits are
         * available: e >= 0 keeps S = 4 and shifts R by e + 2, while e < 0
         * shifts S by 2 - e and R by 2. */
        unsigned shift = exponent >= 0 ? 2u : (unsigned)(-exponent) + 2u;

        ls_rt_big_from_u64(&r, mantissa);
        ls_rt_big_shl(&r, exponent >= 0 ? (unsigned)exponent + 2u : 2u);
        ls_rt_big_from_u64(&s, 1u);
        ls_rt_big_shl(&s, shift);
        ls_rt_big_from_u64(&m_plus, 1u);
        ls_rt_big_shl(&m_plus, exponent >= 0 ? (unsigned)exponent + 1u : 1u);
        ls_rt_big_copy(&m_minus, &m_plus);
        if (power_of_two && biased > 1) {
            ls_rt_big_shr1(&m_minus);
        }
        if (scale >= 0) {
            ls_rt_big_mul_pow10(&s, scale);
        } else {
            ls_rt_big_mul_pow10(&r, -scale);
            ls_rt_big_mul_pow10(&m_plus, -scale);
            ls_rt_big_mul_pow10(&m_minus, -scale);
        }
        if (ls_rt_big_cmp(&r, &s) < 0) {
            scale -= 1;
            continue;
        }
        {
            LsRtBig s10;
            ls_rt_big_copy(&s10, &s);
            ls_rt_big_mul_small(&s10, 10u);
            if (ls_rt_big_cmp(&r, &s10) >= 0) {
                scale += 1;
                continue;
            }
        }
        break;
    }

    /* S never changes inside the digit loop, so the multiples used to extract
     * each decimal digit are computed once. */
    ls_rt_big_copy(&s2, &s);
    ls_rt_big_shl(&s2, 1u);
    ls_rt_big_copy(&s4, &s2);
    ls_rt_big_shl(&s4, 1u);
    ls_rt_big_copy(&s8, &s4);
    ls_rt_big_shl(&s8, 1u);

    for (;;) {
        unsigned digit = 0;
        bool trunc_ok;
        bool round_ok;
        LsRtBig upper_remainder;
        int low_cmp;
        int high_cmp;

        if (ls_rt_big_cmp(&r, &s8) >= 0) {
            ls_rt_big_sub(&r, &s8);
            digit = 8;
        }
        if (ls_rt_big_cmp(&r, &s4) >= 0) {
            ls_rt_big_sub(&r, &s4);
            digit += 4;
        }
        if (ls_rt_big_cmp(&r, &s2) >= 0) {
            ls_rt_big_sub(&r, &s2);
            digit += 2;
        }
        if (ls_rt_big_cmp(&r, &s) >= 0) {
            ls_rt_big_sub(&r, &s);
            digit += 1;
        }
        low_cmp = ls_rt_big_cmp(&r, &m_minus);
        ls_rt_big_copy(&upper_remainder, &s);
        ls_rt_big_sub(&upper_remainder, &r);
        high_cmp = ls_rt_big_cmp(&upper_remainder, &m_plus);
        trunc_ok = inclusive ? low_cmp <= 0 : low_cmp < 0;
        round_ok = inclusive ? high_cmp <= 0 : high_cmp < 0;
        if (trunc_ok || round_ok) {
            bool round_up;
            if (trunc_ok && round_ok) {
                LsRtBig twice;
                int half_cmp;
                ls_rt_big_copy(&twice, &r);
                ls_rt_big_mul_small(&twice, 2u);
                half_cmp = ls_rt_big_cmp(&twice, &s);
                if (half_cmp > 0) {
                    round_up = true;
                } else if (half_cmp < 0) {
                    round_up = false;
                } else {
                    round_up = (digit & 1u) != 0; /* ties to even digit */
                }
            } else {
                round_up = round_ok;
            }
            digits[ndigits] = (char)('0' + (int)digit);
            ndigits += 1;
            if (round_up) {
                ls_rt_digits_round_up(digits, &ndigits, &scale);
            }
            break;
        }
        if (ndigits + 1 >= (size_t)max_digits) {
            /* Every representation of max_digits digits round-trips; round the
             * last generated digit to nearest to stay with printf. */
            LsRtBig twice;
            int half_cmp;
            ls_rt_big_copy(&twice, &r);
            ls_rt_big_mul_small(&twice, 2u);
            half_cmp = ls_rt_big_cmp(&twice, &s);
            digits[ndigits] = (char)('0' + (int)digit);
            ndigits += 1;
            if (half_cmp > 0 || (half_cmp == 0 && (digit & 1u) != 0)) {
                ls_rt_digits_round_up(digits, &ndigits, &scale);
            }
            break;
        }
        digits[ndigits] = (char)('0' + (int)digit);
        ndigits += 1;
        ls_rt_big_mul_small(&r, 10u);
        ls_rt_big_mul_small(&m_plus, 10u);
        ls_rt_big_mul_small(&m_minus, 10u);
    }
    while (ndigits > 1 && digits[ndigits - 1] == '0') {
        ndigits -= 1;
    }
    *out_ndigits = ndigits;
    *out_exponent = scale;
}

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
        uint32_t bits;
        float value32 = (float)value;
        memcpy(&bits, &value32, sizeof bits);
        ls_rt_shortest_float((uint64_t)(bits & 0x7FFFFFFFu), true, digits,
                             &ndigits, &exponent);
    } else {
        uint64_t bits;
        memcpy(&bits, &value, sizeof bits);
        ls_rt_shortest_float(bits & 0x7FFFFFFFFFFFFFFFULL, false, digits,
                             &ndigits, &exponent);
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

/* Rust str Debug quoting: double quotes and backslashes are escaped, the short
 * escapes are used for \t, \r and \n, every scalar Rust considers non-printable
 * (Cc, Cf, Cs, Co, Cn, Zl, Zp and all mark categories) is written as \u{...},
 * and everything else is emitted as raw UTF-8. The buffer is always valid UTF-8,
 * so the loop steps scalar by scalar instead of byte by byte. */
LsString* ls_fmt_debug_string(LsString* acc, const LsString* v) {
    size_t i = 0;

    acc = ls_fmt_append(acc, "\"", 1);
    while (i < v->len) {
        const unsigned char* bytes = (const unsigned char*)v->data + i;
        size_t length = ls_rt_utf8_decode_length(bytes, v->len - i);
        uint32_t scalar;

        if (length == 0) {
            length = 1; /* defensive: buffers are always valid UTF-8 */
        }
        scalar = ls_rt_utf8_scalar_at(bytes);
        switch (scalar) {
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
            if (ls_rt_unicode_char_is_printable(scalar)) {
                acc = ls_fmt_append(acc, (const char*)bytes, length);
            } else {
                char buf[16];
                int written =
                    snprintf(buf, sizeof buf, "\\u{%x}", (unsigned)scalar);
                if (written > 0) {
                    acc = ls_fmt_append(acc, buf, (size_t)written);
                }
            }
            break;
        }
        i += length;
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
}

void ls_println(const LsString* s) {
    ls_rt_configure_stdio();
    if (s != NULL && s->len != 0) {
        (void)fwrite(s->data, 1, s->len, stdout);
    }
    (void)fputc('\n', stdout);
}

void ls_eprint(const LsString* s) {
    ls_rt_configure_stdio();
    if (s != NULL && s->len != 0) {
        (void)fwrite(s->data, 1, s->len, stderr);
    }
}

void ls_eprintln(const LsString* s) {
    ls_rt_configure_stdio();
    if (s != NULL && s->len != 0) {
        (void)fwrite(s->data, 1, s->len, stderr);
    }
    (void)fputc('\n', stderr);
}

void ls_print_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stdout);
    }
}

void ls_println_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stdout);
    }
    (void)fputc('\n', stdout);
}

void ls_eprint_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stderr);
    }
}

void ls_eprintln_cstr(const char* cstr) {
    ls_rt_configure_stdio();
    if (cstr != NULL) {
        (void)fwrite(cstr, 1, strlen(cstr), stderr);
    }
    (void)fputc('\n', stderr);
}

void ls_print_flush(void) {
    ls_rt_configure_stdio();
    fflush(stdout);
    fflush(stderr);
}

/* ====================================================================== */
/* Characters                                                             */
/* ====================================================================== */

/* char Display: the scalar encoded as UTF-8, no quotes. Width/alignment is
 * applied by ls_fmt_apply_width() in the generated code. */
LsString* ls_fmt_char(LsString* acc, uint32_t v) {
    char bytes[4];
    size_t length = ls_rt_utf8_encode_scalar(v, bytes);
    return ls_fmt_append(acc, bytes, length);
}

/* True when Rust's char Debug prints the scalar between the quotes without an
 * escape. Rust escapes Cc, Cf, Cs, Co, Cn, Zl, Zp and the mark categories; the
 * generated Unicode table in lsaca_rt_unicode.c provides the exact predicate. */
static bool ls_rt_fmt_char_printable(uint32_t v) {
    if (v < 0x20u || v == 0x7Fu) {
        return false;
    }
    if (v < 0x7Fu) {
        return true;
    }
    return ls_rt_unicode_char_is_printable(v);
}

/* char Debug: single quotes with Rust's char escaping rules: the short escapes
 * for tab, carriage return, newline, backslash and single quote; printable
 * scalars verbatim as UTF-8; everything else as \u{...}. */
LsString* ls_fmt_char_debug(LsString* acc, uint32_t v) {
    char ascii[16];
    char bytes[4];
    size_t length;
    int written;

    acc = ls_fmt_append(acc, "'", 1);
    switch (v) {
    case '\t':
        return ls_fmt_append(acc, "\\t'", 3);
    case '\r':
        return ls_fmt_append(acc, "\\r'", 3);
    case '\n':
        return ls_fmt_append(acc, "\\n'", 3);
    case '\\':
        return ls_fmt_append(acc, "\\\\'", 3);
    case '\'':
        return ls_fmt_append(acc, "\\''", 3);
    default:
        break;
    }
    if (ls_rt_fmt_char_printable(v)) {
        length = ls_rt_utf8_encode_scalar(v, bytes);
        acc = ls_fmt_append(acc, bytes, length);
    } else {
        written = snprintf(ascii, sizeof ascii, "\\u{%x}", (unsigned)v);
        if (written > 0) {
            acc = ls_fmt_append(acc, ascii, (size_t)written);
        }
    }
    return ls_fmt_append(acc, "'", 1);
}


