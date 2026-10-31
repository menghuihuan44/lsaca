/* lsaca C17 runtime: Unicode tables and UTF-8 primitives.
 *
 * This header is shared by the runtime translation units and by the private
 * contract in lsaca_rt_internal.h. The public generated-code ABI stays in
 * lsaca_rt.h; everything here is runtime-internal.
 *
 * The Unicode lookup tables are generated deterministically from Python's
 * unicodedata module by scripts/gen_unicode_tables.py and committed in
 * lsaca_rt_unicode.c, so building or running a program never needs Python.
 */

#ifndef LSACA_RT_UNICODE_H
#define LSACA_RT_UNICODE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Strict UTF-8 validation of a whole byte sequence.
 *
 * Returns true iff data[0..len) is a well-formed UTF-8 string: truncated
 * sequences, stray continuation bytes, overlong forms, UTF-16 surrogate
 * halves (U+D800..U+DFFF) and scalars above U+10FFFF are rejected. The empty
 * sequence is valid. The bytes are not modified and are not required to be
 * NUL terminated; data may be NULL only when len is zero. */
bool ls_rt_utf8_validate(const char* data, size_t len);

/* Encode one Unicode scalar value as UTF-8 into out[0..4) and return the number
 * of bytes written (1..4). Invalid scalars (UTF-16 surrogate halves and values
 * above U+10FFFF) are encoded as U+FFFD REPLACEMENT CHARACTER; the language's
 * char type only admits valid scalars, so this only guards against runtime
 * bugs. Defined in lsaca_rt_string.c. */
size_t ls_rt_utf8_encode_scalar(uint32_t scalar, char out[4]);

/* Decode helpers shared with the formatting translation unit.
 *
 * ls_rt_utf8_decode_length returns the length in bytes (1..4) of the
 * well-formed UTF-8 sequence starting at p, or 0 when the remaining bytes are
 * truncated, invalid, an overlong form, a UTF-16 surrogate half or above
 * U+10FFFF; it never reads past p[remaining). ls_rt_utf8_scalar_at decodes the
 * scalar stored at p, which must point at the start of a well-formed sequence.
 * Both are defined in lsaca_rt_string.c. */
size_t ls_rt_utf8_decode_length(const unsigned char* p, size_t remaining);
uint32_t ls_rt_utf8_scalar_at(const unsigned char* p);

/* True when Rust's Debug formatting prints the scalar verbatim (no escape).
 * Rust escapes the Cc, Cf, Cs, Co, Cn, Zl and Zp general categories and every
 * mark category (Mn/Mc/Me); everything else is printed as UTF-8. Implemented
 * from the generated Unicode category table in lsaca_rt_unicode.c. */
bool ls_rt_unicode_char_is_printable(uint32_t scalar);

/* Maximum number of scalars a single case mapping may expand to. The generator
 * fails if Unicode ever needs more than this. */
#define LS_RT_UNICODE_MAX_EXPANSION 4

/* Full Unicode default case mapping of one scalar.
 *
 * Writes the mapped scalars to out (which must have room for
 * LS_RT_UNICODE_MAX_EXPANSION entries) and returns the number written (1..4).
 * A scalar without a mapping is copied through unchanged. One-to-many mappings
 * are supported (U+00DF -> "SS", U+0130 -> "i" + U+0307). The mapping is the
 * unconditional default mapping; language-specific tailoring (for example
 * Turkish dotless i) is not applied. Defined in lsaca_rt_unicode.c. */
size_t ls_rt_unicode_upper(uint32_t scalar, uint32_t out[LS_RT_UNICODE_MAX_EXPANSION]);
size_t ls_rt_unicode_lower(uint32_t scalar, uint32_t out[LS_RT_UNICODE_MAX_EXPANSION]);

/* True when the scalar has the Unicode White_Space property. Defined in
 * lsaca_rt_unicode.c and used by String.trim(). */
bool ls_rt_unicode_is_whitespace(uint32_t scalar);

#endif /* LSACA_RT_UNICODE_H */
