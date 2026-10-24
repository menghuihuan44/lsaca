/* lsaca C17 runtime: reference counting (ORC), strings, vectors, formatting,
 * checked integer arithmetic, panics, process/IO and math helpers.
 *
 * This header is the ABI contract between generated C code and the runtime.
 * See README.md ("Memory management (ORC)") and the section comments below for
 * the full description.
 *
 * Conventions:
 *   - A reference counted object is an ls_alloc() payload. ls_alloc() returns
 *     the payload pointer; the reference count header lives right before it.
 *     The initial count is 1. ls_retain()/ls_release() accept NULL.
 *   - LsString values are *handles*: a value of type "LsString *". The struct
 *     LsString is opaque to generated code; use ls_str_* accessors.
 *   - LsVec_String / LsOpt_String / LsRes_String_String are plain by-value
 *     structs (frozen layouts, see below). Copied by the codegen-emitted
 *     copy/release helpers, never by the runtime.
 *   - All runtime symbols use the "ls_" prefix; generated code uses "lsaca_".
 *   - Strings, vectors and Options are byte/ownership containers; the runtime
 *     never validates UTF-8. Case conversion and trim are ASCII only, other
 *     bytes are copied through unchanged.
 *   - The runtime is not thread safe (the language has no threads).
 *   - Define LSACA_DEBUG (any value) when compiling the runtime and generated
 *     code in a debug configuration to enable leak checking. The generated
 *     CMake project defines it for the Debug configuration.
 */

#ifndef LSACA_RT_H
#define LSACA_RT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Portable noreturn marker for C17 compilers. */
#if defined(_MSC_VER)
#define LS_NORETURN __declspec(noreturn)
#else
#define LS_NORETURN _Noreturn
#endif

/* ====================================================================== */
/* Reference counted allocations                                          */
/* ====================================================================== */

/* Allocate a zeroed, reference counted payload of "size" bytes with natural
 * alignment (at least max_align_t, which covers every language type). The
 * payload pointer is returned; the reference count starts at 1.
 * Never returns NULL: out of memory terminates the program with an error. */
void* ls_alloc(size_t size);

/* Increment the reference count and return the same payload pointer.
 * NULL is a no-op (returns NULL). */
void* ls_retain(void* obj);

/* Decrement the reference count, freeing the payload at zero. NULL is a
 * no-op. */
void ls_release(void* obj);

/* Current reference count of a payload pointer, 0 for NULL. */
size_t ls_refcount(const void* obj);

/* ====================================================================== */
/* Process lifetime                                                       */
/* ====================================================================== */

/* Store the process arguments for ls_args(). Returns 0. The generated main()
 * calls this before the user's main function. */
int32_t ls_runtime_init(int argc, char** argv);

/* Flush the standard streams and, when the runtime is compiled with
 * LSACA_DEBUG or LSACA_DEBUG_RC defined, walk all live reference counted
 * objects and report leaks to stderr as
 *     lsaca: warning: N reference-counted allocation(s) leaked
 * followed by up to a few "lsaca:   leaked allocation of S bytes" lines.
 * Without the flag nothing is printed (a correct program never writes to
 * stderr here). Called right before main() returns. */
void ls_runtime_shutdown(void);

/* ====================================================================== */
/* Strings                                                                */
/* ====================================================================== */

/* Opaque string object. A String value in generated code is LsString*: a
 * reference counted allocation holding the length, the capacity and the
 * NUL-terminated byte buffer. Generated code must treat this struct as
 * opaque and only use the accessors below. */
typedef struct LsString {
    size_t len;
    size_t cap;
    char* data; /* NUL terminated; never NULL */
} LsString;

/* Constructors: the returned handle is owned by the caller. */
LsString* ls_str_new(void);                            /* empty */
LsString* ls_str_cstr(const char* cstr);               /* copies a C string */
LsString* ls_str_from_parts(const char* bytes, size_t len); /* copies len bytes */
LsString* ls_str_with_capacity(size_t capacity);       /* empty string with room */

/* Observe a String (borrowed; the handle stays owned by the caller). */
size_t ls_str_len(const LsString* s);
bool ls_str_is_empty(const LsString* s);
const char* ls_str_data(const LsString* s); /* NUL terminated byte view */
bool ls_str_eq(const LsString* a, const LsString* b);
int32_t ls_str_cmp(const LsString* a, const LsString* b); /* byte order */

/* Ownership: a String handle is a reference counted payload, so the generic
 * ls_retain()/ls_release() helpers below are THE way to copy/release it
 * (cast retain's void* result back to LsString*):
 *
 *     LsString* copy = (LsString*)ls_retain(value);
 *     ls_release(value);
 *
 * ls_str_clone() is a different operation: an independent deep copy.
 * A String argument to a function is borrowed: the callee never releases it. */
LsString* ls_str_clone(const LsString* s);

/* Concatenation: arguments are borrowed, the result is a new owned string. */
LsString* ls_str_concat(const LsString* a, const LsString* b);

/* Copy-on-write mutators. They CONSUME the caller's reference to "s":
 * the function may have to allocate a new string object (growing the buffer,
 * or detaching from a shared buffer when the reference count is > 1), in
 * which case it releases "s" and returns a new handle. The caller must
 * always store the returned handle (and never drop the old one again):
 *
 *     s = ls_str_push_str(s, suffix);
 *
 * The handle is returned unchanged when the mutation can happen in place.
 * "suffix"/"extra" arguments are borrowed and are never modified. */
LsString* ls_str_push_str(LsString* s, const LsString* suffix);
LsString* ls_str_push_cstr(LsString* s, const char* suffix);
LsString* ls_str_push_char(LsString* s, char c); /* appends one byte */
LsString* ls_str_clear(LsString* s);             /* len = 0, keeps capacity */
LsString* ls_str_reserve(LsString* s, size_t additional);

/* Searches: the empty needle is found at index 0. find() returns the byte
 * index of the first match or -1. */
bool ls_str_contains(const LsString* haystack, const LsString* needle);
bool ls_str_starts_with(const LsString* s, const LsString* prefix);
bool ls_str_ends_with(const LsString* s, const LsString* suffix);
int64_t ls_str_find(const LsString* haystack, const LsString* needle);

/* Transformations: return a new owned string, the input is borrowed. */
LsString* ls_str_replace(const LsString* s, const LsString* from,
                         const LsString* to);
LsString* ls_str_to_uppercase(const LsString* s);
LsString* ls_str_to_lowercase(const LsString* s);
LsString* ls_str_trim(const LsString* s); /* ASCII whitespace */

/* Bounds-checked; panic with the given source location when out of range.
 * substring(s, start, len): start + len must be <= s.len. */
LsString* ls_str_substring(const LsString* s, size_t start, size_t len,
                           const char* file, int32_t line, int32_t col);
LsString* ls_str_repeat(const LsString* s, size_t count, const char* file,
                        int32_t line, int32_t col);

/* Split is declared in the "Frozen shared instantiations" section below,
 * because it returns LsVec_String. */

/* Strict Rust-style parsing: the whole string must be consumed, no leading or
 * trailing whitespace is allowed, and overflow returns false (never panics).
 * On success the value is stored through "out". */
bool ls_str_parse_i64(const LsString* s, int64_t* out);
bool ls_str_parse_f64(const LsString* s, double* out);

/* ====================================================================== */
/* Generic vector primitives (for codegen-generated LsVec_<T>)            */
/* ====================================================================== */
/* Every monomorphized vector is a by-value struct:
 *
 *     typedef struct LsVec_i64 { size_t len; size_t cap; int64_t* data; }
 *     LsVec_i64;
 *
 * The data buffer is a raw malloc()'d array (NOT reference counted); the
 * codegen-emitted copy helper deep-copies the buffer and its elements, and
 * the release helper releases the elements and frees the buffer. These
 * primitives operate on the raw buffer and are element-type agnostic; they
 * only move bytes (they never retain or release owned element payloads).
 *
 * The helpers reallocate and update *data and *cap. elem_size is the C
 * sizeof of the element type. */

/* Allocate a zeroed buffer of "capacity" elements (NULL for capacity 0). */
void* ls_vec_alloc(size_t capacity, size_t elem_size);

/* Ensure room for len + 1 elements (the push fast path). */
void ls_vec_grow(void** data, size_t* cap, size_t len, size_t elem_size);

/* Ensure room for at least "needed" elements. */
void ls_vec_reserve(void** data, size_t* cap, size_t len, size_t needed,
                    size_t elem_size);

/* Bitwise copy of a buffer of len elements (bitwise-copy element types only;
 * owned elements need a retaining loop generated by codegen). */
void* ls_vec_clone_bytes(const void* data, size_t len, size_t elem_size);

/* Byte-level mutation helpers; the caller checks indices (the language-level
 * vector methods bounds check before calling). "value" may alias the vector
 * buffer: the bytes are copied to a temporary before any reallocation. */
void ls_vec_insert_bytes(void** data, size_t* len, size_t* cap, size_t index,
                         const void* value, size_t elem_size);
void ls_vec_remove_bytes(void* data, size_t* len, size_t index, void* out,
                         size_t elem_size);
void ls_vec_extend_bytes(void** data, size_t* len, size_t* cap, const void* src,
                         size_t src_len, size_t elem_size);
void ls_vec_reverse_bytes(void* data, size_t len, size_t elem_size);

/* Release a raw vector buffer (does not touch the elements). */
void ls_vec_free_bytes(void* data);

/* ====================================================================== */
/* Frozen shared instantiations                                           */
/* ====================================================================== */
/* These layouts are frozen by the runtime ABI (see README.md); generated
 * code never redefines the structs, only emits language-level helpers for
 * them. */

/* Vec<String>: data is a malloc'd array of owned String handles. */
typedef struct LsVec_String {
    size_t len;
    size_t cap;
    LsString** data;
} LsVec_String;

/* Option<String>: tag 0 = None, tag 1 = Some. */
typedef struct LsOpt_String {
    int32_t tag;
    union {
        LsString* some;
    } u;
} LsOpt_String;

/* Result<String, String>: tag 0 = Ok, tag 1 = Err. */
typedef struct LsRes_String_String {
    int32_t tag;
    union {
        LsString* ok;
        LsString* err;
    } u;
} LsRes_String_String;

/* LsVec_String operations. The vector owns one reference per element. */
LsVec_String ls_vec_string_new(void);
/* Split on a separator (declared here because it returns LsVec_String). Rust
 * semantics for the empty separator (individual UTF-8 scalar values with
 * leading/trailing empty pieces) are implemented; as everywhere else in this
 * runtime, "UTF-8 scalar value" means one byte for ASCII and a maximal valid
 * UTF-8 sequence for non-ASCII input. */
LsVec_String ls_str_split(const LsString* s, const LsString* separator);
LsVec_String ls_vec_string_with_capacity(size_t capacity);
void ls_vec_string_push(LsVec_String* v, const LsString* value);
void ls_vec_string_insert(LsVec_String* v, size_t index, const LsString* value);
void ls_vec_string_extend(LsVec_String* v, const LsVec_String* other);
LsString* ls_vec_string_remove(LsVec_String* v, size_t index); /* ownership out */
bool ls_vec_string_pop(LsVec_String* v, LsString** out);
/* Vec::get semantics: copies (retains) element index into *out, returns false
 * when out of bounds. */
bool ls_vec_string_get(const LsVec_String* v, size_t index, LsString** out);
void ls_vec_string_clear(LsVec_String* v);
void ls_vec_string_reverse(LsVec_String* v);
/* Release the vector: releases every element and frees the data buffer. */
void ls_vec_string_free(LsVec_String* v);

/* Construction helpers for the frozen shared instantiations. These do NOT
 * retain the payload: ownership of "some"/"ok"/"err" transfers into the
 * returned struct and is released by the codegen-emitted release helper. */
static inline LsOpt_String ls_opt_string_none(void) {
    LsOpt_String o;
    o.tag = 0;
    o.u.some = NULL;
    return o;
}
static inline LsOpt_String ls_opt_string_some(LsString* some) {
    LsOpt_String o;
    o.tag = 1;
    o.u.some = some;
    return o;
}
static inline LsRes_String_String ls_res_string_ok(LsString* ok) {
    LsRes_String_String r;
    r.tag = 0;
    r.u.err = NULL; /* inactive branch first: both alias the same storage */
    r.u.ok = ok;
    return r;
}
static inline LsRes_String_String ls_res_string_err(LsString* err) {
    LsRes_String_String r;
    r.tag = 1;
    r.u.ok = NULL;
    r.u.err = err;
    return r;
}

/* ====================================================================== */
/* Formatting                                                             */
/* ====================================================================== */
/* A format! expression builds a string from pieces:
 *
 *     LsString* acc = ls_format_begin(16);
 *     acc = ls_format_push_str(acc, literal);   // borrowed piece
 *     acc = ls_fmt_i64(acc, value);
 *     ...
 *
 * The accumulator handle is CONSUMED by every formatter: the function may
 * reallocate it (or detach it from a shared buffer) and return the new
 * handle, releasing the old one. Always assign the result back. The final
 * result is owned by the caller and is released with ls_release(). */

LsString* ls_format_begin(size_t capacity_hint);
LsString* ls_format_push_str(LsString* acc, const LsString* piece);
LsString* ls_format_push_char(LsString* acc, char c);
LsString* ls_format_push_cstr(LsString* acc, const char* cstr);
LsString* ls_fmt_i64(LsString* acc, int64_t v);
LsString* ls_fmt_u64(LsString* acc, uint64_t v);
LsString* ls_fmt_bool(LsString* acc, bool v);
/* Shortest decimal representation that round-trips to the same bits.
 * f64 and f32 are formatted independently (0.1f32 prints as "0.1"). */
LsString* ls_fmt_f64(LsString* acc, double v);
LsString* ls_fmt_f32(LsString* acc, float v);
/* Debug ({:?}) formatting. */
LsString* ls_fmt_debug_string(LsString* acc, const LsString* v);
LsString* ls_fmt_f64_debug(LsString* acc, double v);
LsString* ls_fmt_f32_debug(LsString* acc, float v);

/* ====================================================================== */
/* Output                                                                 */
/* ====================================================================== */
/* Print helpers take a BORROWED string: they do not release it. Every
 * helper flushes its stream after writing so that stdout/stderr ordering is
 * sane when the program is piped. */
void ls_print(const LsString* s);
void ls_println(const LsString* s);
void ls_eprint(const LsString* s);
void ls_eprintln(const LsString* s);
/* C-string variants, for printing string literals without allocating a
 * String object first. */
void ls_print_cstr(const char* cstr);
void ls_println_cstr(const char* cstr);
void ls_eprint_cstr(const char* cstr);
void ls_eprintln_cstr(const char* cstr);
void ls_print_flush(void);

/* ====================================================================== */
/* Panics                                                                 */
/* ====================================================================== */
/* Panics print to stderr
 *
 *     thread 'main' panicked at <file>:<line>:<col>:
 *     <message>
 *
 * flush stdout, then flush stderr and exit(101). file may be NULL or empty
 * (release builds without location information); line/col are then ignored
 * by the printer but the header is still printed.
 *
 * ls_panic_at takes the message as a borrowed String handle; ls_panic_cstr
 * takes a static C string (used for compiler-generated messages). */

LS_NORETURN void ls_panic_at(const char* file, int32_t line, int32_t col,
                             const LsString* msg);
LS_NORETURN void ls_panic_cstr(const char* file, int32_t line, int32_t col,
                               const char* msg);
/* Bounds panic: "index out of bounds: the len is <len> but the index is
 * <index>" (Rust's exact message; both values decimal). */
LS_NORETURN void ls_panic_bounds(const char* file, int32_t line, int32_t col,
                                 int64_t len, int64_t index);
/* "attempt to divide by zero". */
LS_NORETURN void ls_panic_div_zero(const char* file, int32_t line, int32_t col);
/* Overflow panic: "attempt to <op> with overflow", where op is one of "add",
 * "subtract", "multiply", "divide", "remainder", "shift left", "shift
 * right", "negate". (The checked arithmetic functions below use the same
 * messages directly so that codegen does not have to pass op strings.) */
LS_NORETURN void ls_panic_overflow(const char* file, int32_t line, int32_t col,
                                   const char* op);

/* ====================================================================== */
/* Checked integer arithmetic (debug builds call these; overflow panics)  */
/* ====================================================================== */
/* One family per integer type and operation. The generated code passes the
 * source location of the operation as the last three arguments. Messages are
 * Rust-style:
 *   "attempt to add with overflow", "attempt to subtract with overflow",
 *   "attempt to multiply with overflow", "attempt to divide by zero",
 *   "attempt to divide with overflow",
 *   "attempt to calculate the remainder with a divisor of zero",
 *   "attempt to calculate the remainder with overflow",
 *   "attempt to shift left with overflow", "attempt to shift right with
 *   overflow", "attempt to negate with overflow".
 *
 * Shift counts are int64_t for every type; a negative count or a count >= the
 * bit width panics. Unsigned types have no neg operation. */

#define LS_RT_DECL_CHECKED_SIGNED(sfx, ctype)                                     \
    ctype ls_add_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_sub_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_mul_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_div_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_rem_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_shl_##sfx##_checked(ctype a, int64_t b, const char* file,            \
                                 int32_t line, int32_t col);                      \
    ctype ls_shr_##sfx##_checked(ctype a, int64_t b, const char* file,            \
                                 int32_t line, int32_t col);                      \
    ctype ls_neg_##sfx##_checked(ctype a, const char* file, int32_t line,         \
                                 int32_t col);

#define LS_RT_DECL_CHECKED_UNSIGNED(sfx, ctype)                                   \
    ctype ls_add_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_sub_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_mul_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_div_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_rem_##sfx##_checked(ctype a, ctype b, const char* file,              \
                                 int32_t line, int32_t col);                      \
    ctype ls_shl_##sfx##_checked(ctype a, int64_t b, const char* file,            \
                                 int32_t line, int32_t col);                      \
    ctype ls_shr_##sfx##_checked(ctype a, int64_t b, const char* file,            \
                                 int32_t line, int32_t col);

LS_RT_DECL_CHECKED_SIGNED(i8, int8_t)
LS_RT_DECL_CHECKED_SIGNED(i16, int16_t)
LS_RT_DECL_CHECKED_SIGNED(i32, int32_t)
LS_RT_DECL_CHECKED_SIGNED(i64, int64_t)
LS_RT_DECL_CHECKED_UNSIGNED(u8, uint8_t)
LS_RT_DECL_CHECKED_UNSIGNED(u16, uint16_t)
LS_RT_DECL_CHECKED_UNSIGNED(u32, uint32_t)
LS_RT_DECL_CHECKED_UNSIGNED(u64, uint64_t)

#undef LS_RT_DECL_CHECKED_SIGNED
#undef LS_RT_DECL_CHECKED_UNSIGNED

/* ====================================================================== */
/* Process and IO                                                         */
/* ====================================================================== */

/* Program arguments EXCLUDING the executable name (language semantics:
 * args()[0] is the first argument after the program name). Materialized on
 * every call. The caller owns the returned vector (release with
 * ls_vec_string_free). */
LsVec_String ls_args(void);

/* Read one line from stdin without the trailing newline ("\r\n" is
 * normalized). Returns tag 0 (None) on EOF, tag 1 (Some) with an owned string
 * otherwise. */
LsOpt_String ls_read_line(void);

/* Read a whole file (binary safe, no UTF-8 validation). On error the result
 * has tag 1 (Err) and an owned OS error message such as
 * "No such file or directory (os error 2)". */
LsRes_String_String ls_read_file(const LsString* path);

/* Write a whole file (truncates it first). On success tag 0 (Ok) with an empty
 * value string; on error tag 1 (Err) with an owned message. */
LsRes_String_String ls_write_file(const LsString* path, const LsString* contents);

/* Flush stdout/stderr and exit with "code". Does not run ls_runtime_shutdown
 * (mirrors Rust process::exit). */
LS_NORETURN void ls_exit(int32_t code);

/* ====================================================================== */
/* Math helpers                                                           */
/* ====================================================================== */
/* Integer abs/min/max; the plain ls_abs_<T> wraps on the type minimum in
 * release builds, ls_abs_<T>_checked panics with "attempt to negate with
 * overflow" in debug builds. Unsigned abs is the identity. */
#define LS_RT_DECL_INT_MATH(sfx, ctype)                                       \
    ctype ls_abs_##sfx(ctype v);                                              \
    ctype ls_min_##sfx(ctype a, ctype b);                                     \
    ctype ls_max_##sfx(ctype a, ctype b);

LS_RT_DECL_INT_MATH(i8, int8_t)
LS_RT_DECL_INT_MATH(i16, int16_t)
LS_RT_DECL_INT_MATH(i32, int32_t)
LS_RT_DECL_INT_MATH(i64, int64_t)
LS_RT_DECL_INT_MATH(u8, uint8_t)
LS_RT_DECL_INT_MATH(u16, uint16_t)
LS_RT_DECL_INT_MATH(u32, uint32_t)
LS_RT_DECL_INT_MATH(u64, uint64_t)

#undef LS_RT_DECL_INT_MATH

int8_t ls_abs_i8_checked(int8_t v, const char* file, int32_t line, int32_t col);
int16_t ls_abs_i16_checked(int16_t v, const char* file, int32_t line,
                           int32_t col);
int32_t ls_abs_i32_checked(int32_t v, const char* file, int32_t line,
                           int32_t col);
int64_t ls_abs_i64_checked(int64_t v, const char* file, int32_t line,
                           int32_t col);

/* Floating point helpers (libm). The f32 variants compute in single
 * precision so that f32 programs match Rust f32 semantics. */
float ls_abs_f32(float v);
double ls_abs_f64(double v);
float ls_min_f32(float a, float b);
double ls_min_f64(double a, double b);
float ls_max_f32(float a, float b);
double ls_max_f64(double a, double b);

float ls_sqrt_f32(float v);
double ls_sqrt_f64(double v);
float ls_pow_f32(float base, float exponent);
double ls_pow_f64(double base, double exponent);
float ls_floor_f32(float v);
double ls_floor_f64(double v);
float ls_ceil_f32(float v);
double ls_ceil_f64(double v);
float ls_round_f32(float v);
double ls_round_f64(double v);
float ls_sin_f32(float v);
double ls_sin_f64(double v);
float ls_cos_f32(float v);
double ls_cos_f64(double v);
float ls_tan_f32(float v);
double ls_tan_f64(double v);
float ls_ln_f32(float v);
double ls_ln_f64(double v);
float ls_log10_f32(float v);
double ls_log10_f64(double v);
float ls_exp_f32(float v);
double ls_exp_f64(double v);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* LSACA_RT_H */
