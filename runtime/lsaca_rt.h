/* lsaca C17 runtime: reference counting (ORC), strings, vectors, hash maps,
 * formatting, checked integer arithmetic, panics, process/IO and math helpers.
 *
 * This header is the ABI contract between generated C code and the runtime.
 * See README.md ("Memory management (ORC)") and the section comments below for
 * the full description.
 *
 * Conventions:
 *   - A reference counted object is an allocated payload; the reference count
 *     header lives right before it and the initial count is 1.
 *     ls_retain()/ls_release() accept NULL. Strings and other payloads that
 *     cannot form reference cycles use ls_alloc() (unmanaged: no drop
 *     function, no trace function, never a cycle collector node). Payloads
 *     that can form cycles (Rc cells and closure environments in generated
 *     code) use ls_alloc_managed() and carry a compiler generated drop
 *     function and a trace function used by the cycle collector.
 *   - LsString values are *handles*: a value of type "LsString *". The struct
 *     LsString is opaque to generated code; use ls_str_* accessors.
 *   - LsVec_String / LsOpt_String / LsRes_String_String are plain by-value
 *     structs (frozen layouts, see below). Copied by the codegen-emitted
 *     copy/release helpers, never by the runtime.
 *   - All runtime symbols use the "ls_" prefix; generated code uses "lsaca_".
 *   - LsString payloads are always valid UTF-8, and the runtime preserves
 *     that invariant: the character/String methods reject or split only at
 *     scalar boundaries, case conversion and trim use the full Unicode
 *     tables in lsaca_rt_unicode.c, and the IO entry points (read_line,
 *     ls_args, ls_read_file) validate whatever the process provides. Vectors
 *     and Options are plain byte/ownership containers for their element type.
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
#include <string.h>

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
/* Reference counted allocations (ARC + cycle collector)                  */
/* ====================================================================== */

/* Drop function of an ORC-managed allocation. It is called once, immediately
 * before the payload memory is freed, while the payload is still intact. It
 * must release every owned handle stored in the payload (strings and managed
 * handles). Unmanaged ls_alloc() payloads have no drop function. */
typedef void (*LsDropFn)(void* payload);

/* Visitor called by a trace function once for every managed (cycle capable)
 * child handle stored in a payload. */
typedef void (*LsTraceVisit)(void* child);

/* Trace function of an ORC-managed allocation. It visits every managed child
 * handle owned by the payload, without mutating anything and without
 * allocating. A NULL trace means "this payload cannot hold managed children
 * and can never be part of a reference cycle"; such an allocation is never a
 * cycle collector node. */
typedef void (*LsTraceFn)(const void* payload, LsTraceVisit visit);

/* Allocate a zeroed, reference counted payload of "size" bytes with natural
 * alignment (at least max_align_t, which covers every language type). The
 * payload pointer is returned; the reference count starts at 1.
 * This is the UNMANAGED allocation used for strings and other payloads that
 * cannot form cycles: no drop function, no trace function. It is still
 * reference counted, so ls_retain()/ls_release() work as usual.
 * Never returns NULL: out of memory terminates the program with an error. */
void* ls_alloc(size_t size);

/* Allocate a zeroed ORC-managed payload of "size" bytes with the same
 * alignment and initial reference count as ls_alloc(). "drop" may be NULL;
 * "trace" may be NULL (then the payload cannot be part of a cycle and is not
 * a collector node). Otherwise the payload participates in cycle collection:
 * when its managed reference count drops but stays above zero it becomes a
 * collector candidate.
 * Never returns NULL: out of memory terminates the program with an error. */
void* ls_alloc_managed(size_t size, LsDropFn drop, LsTraceFn trace);

/* Increment the reference count and return the same payload pointer.
 * NULL is a no-op (returns NULL). Works for managed and unmanaged payloads. */
void* ls_retain(void* obj);

/* Decrement the reference count, freeing the payload at zero. NULL is a
 * no-op. At zero the drop function (if any) runs while the payload is still
 * intact, then the allocation is freed normally. Under a running collection
 * the collector suppresses the releases it already accounted for, so drop
 * functions always see consistent counts. */
void ls_release(void* obj);

/* Current reference count of a payload pointer, 0 for NULL. */
size_t ls_refcount(const void* obj);

/* Run the cycle collector synchronously: trial-delete candidate objects,
 * determine which candidates are only reachable from garbage cycles and free
 * those cycles. A no-op when nothing is pending. Reentrancy safe: calling it
 * while a collection runs (for example from a drop function) does nothing.
 * The runtime is single threaded; the collector uses no atomics and is not
 * thread safe either. */
void ls_collect_cycles(void);

/* ====================================================================== */
/* Process lifetime                                                       */
/* ====================================================================== */

/* Store the process arguments for ls_args(). Returns 0. The generated main()
 * calls this before the user's main function. */
int32_t ls_runtime_init(int argc, char** argv);

/* Flush the standard streams, run pending cycle collections (bounded number
 * of passes) so that unreachable reference cycles are reclaimed first, and
 * then, when the runtime is compiled with LSACA_DEBUG or LSACA_DEBUG_RC
 * defined, walk all still live reference counted objects and report leaks to
 * stderr as
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
/* Immortal interned string: copies `len` bytes (embedded NULs included) into a
 * fresh LsString whose allocation is immortal. ls_release() only drops the
 * reference count and never destroys the object; the allocation is not counted
 * or reported by the debug leak report. The returned handle is owned by the
 * caller. Generated code builds each distinct literal once at startup and
 * retains the cached handle per use. */
LsString* ls_str_static(const char* data, size_t len);
LsString* ls_str_with_capacity(size_t capacity);       /* empty string with room */
LsString* ls_str_from_scalar(uint32_t scalar);         /* one Unicode scalar */

/* Observe a String (borrowed; the handle stays owned by the caller). */
size_t ls_str_len(const LsString* s);
bool ls_str_is_empty(const LsString* s);
/* Number of Unicode scalar values in the string (not bytes). The buffer is
 * always valid UTF-8, so this never fails. */
int64_t ls_str_char_count(const LsString* s);
/* Copy the scalar with the given character index (0 based, counting Unicode
 * scalar values) into *out. Returns false when the index is out of range. */
bool ls_str_char_at(const LsString* s, int64_t char_index, uint32_t* out);
/* True when the byte offset is 0, s->len, or the start of a UTF-8 sequence. */
bool ls_str_is_char_boundary(const LsString* s, int64_t byte_index);
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
/* Append one Unicode scalar value, encoded as UTF-8 (String::push(char)).
 * Same copy-on-write ownership contract as the other mutators. */
LsString* ls_str_push_scalar(LsString* s, uint32_t scalar);
/* Remove the last Unicode scalar value (String::pop). The handle is consumed
 * and returned like every other copy-on-write mutator. *out_some reports
 * whether the string was non-empty; *out_scalar receives the removed scalar. */
LsString* ls_str_pop_scalar(LsString* s, bool* out_some, uint32_t* out_scalar);
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
/* Full Unicode default case mapping: one scalar may expand to several
 * (for example sharp s upper-cases to "SS"); the buffer stays valid UTF-8. */
LsString* ls_str_to_uppercase(const LsString* s);
LsString* ls_str_to_lowercase(const LsString* s);
/* Trim leading and trailing Unicode White_Space scalars. */
LsString* ls_str_trim(const LsString* s);

/* Bounds-checked; panic with the given source location when out of range.
 * substring(s, start, len): start + len must be <= s.len and both start and
 * start + len must fall on a UTF-8 scalar boundary; the panic messages match
 * Rust's str slicing (out of bounds / not a char boundary). */
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

/* Fast append of one element: the hot path of Vec::push.
 *
 *     ls_vec_push_bytes(&v.data, &v.len, &v.cap, &value, sizeof value);
 *
 * is equivalent to
 *
 *     ls_vec_insert_bytes(&v.data, &v.len, &v.cap, v.len, &value,
 *                         sizeof value);
 *
 * but performs the copy inline when the capacity already has room (the common
 * case) and falls back to the out-of-line helper, which grows the buffer and
 * handles aliasing, when it is full. "value" points at the element to copy and
 * may point into the vector buffer itself. */
static inline void ls_vec_push_bytes(void** data, size_t* len, size_t* cap,
                                     const void* value, size_t elem_size) {
    if (elem_size == 0) {
        *len += 1;
        return;
    }
    if (*len < *cap) {
        memcpy((char*)*data + (*len) * elem_size, value, elem_size);
        *len += 1;
        return;
    }
    ls_vec_insert_bytes(data, len, cap, *len, value, elem_size);
}

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

/* Vec<char>: data is a malloc'd array of uint32_t scalars (frozen layout). */
typedef struct LsVec_char {
    size_t len;
    size_t cap;
    uint32_t* data;
} LsVec_char;

/* Option<char>: tag 0 = None, tag 1 = Some. */
typedef struct LsOpt_char {
    int32_t tag;
    union {
        uint32_t some;
    } u;
} LsOpt_char;

/* Collect every Unicode scalar value into a fresh Vec<char> (owned). */
LsVec_char ls_str_chars(const LsString* s);

static inline LsOpt_char ls_opt_char_none(void) {
    LsOpt_char o;
    o.tag = 0;
    o.u.some = 0;
    return o;
}
static inline LsOpt_char ls_opt_char_some(uint32_t some) {
    LsOpt_char o;
    o.tag = 1;
    o.u.some = some;
    return o;
}

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
/* Hash maps (builtin HashMap<K, V>)                                      */
/* ====================================================================== */
/* LsMapTable is a by-value byte container with six code-generator supplied
 * callbacks. Generated code wraps it in an ORC managed payload
 * (typedef struct LsMap_K_V { LsMapTable table; }) and never touches the table
 * internals directly; the runtime owns only the three slot arrays.
 *
 * Algorithm (frozen contract): a SwissTable style hash map with 16 slot
 * groups.
 *   - Control bytes: one uint8_t per slot followed by 16 mirror bytes, so a
 *     16 slot group can always be loaded as two adjacent uint64_t words. The
 *     value is 0x00..0x7F for an occupied slot (the low 7 bits of the key
 *     hash, called H2), 0x80 for empty and 0xFE for a deleted (tombstone)
 *     slot. The byte masks are computed with portable SWAR arithmetic on two
 *     64-bit loads (no SSE, no unaligned or strict-aliasing tricks, no
 *     platform intrinsics).
 *   - Probing: the group index is (hash >> 7) & (cap / 16 - 1), so a group is
 *     16 consecutive slots; groups are probed forward (wrapping) until an
 *     empty control byte stops the search. Within a group every matching H2
 *     candidate is compared with eq_key().
 *   - Capacity is a power of two, at least 16, and 0 before the first insert.
 *     The load factor is 7/8: the table grows when growth_left reaches zero;
 *     growth doubles the capacity when len * 8 >= cap * 7 and otherwise
 *     rehashes at the same capacity (dropping tombstones).
 *   - Iteration walks slot indices in order. That order is deterministic for a
 *     given sequence of operations but is an implementation detail, not part of
 *     the language contract.
 *
 * Ownership: an occupied slot owns its key and value byte blobs.
 *   - ls_map_insert() adopts (moves) both incoming blobs. If the key already
 *     exists, the stored value is copied through copy_value() into
 *     out_old_value (when non-NULL) and released, the incoming value is
 *     adopted, and the caller's duplicate key is released through drop_key()
 *     (ownership of it was transferred to the map). The function returns true
 *     iff an old value was written out.
 *   - ls_map_remove() moves the stored value bytes into out_value (or releases
 *     the value when out_value is NULL) and releases the stored key.
 *   - ls_map_clear() releases every stored key and value but keeps capacity.
 *   - ls_map_get_ptr() and ls_map_iter_next() return borrowed views that are
 *     valid until the next mutation of the table.
 *   - copy_key()/copy_value() are never called on the adoption path; code
 *     generation uses copy_key() for the keys() snapshot and copy_value() for
 *     get() and old-value outputs.
 * The generated wrapper's drop helper must call ls_map_clear() and then free()
 * keys, values and ctrl; all three pointers are NULL before the first insert.
 *
 * Preconditions: the ls_map_init() callbacks are all non-NULL and key_size /
 * value_size are the non-zero C sizeof of the language key/value types. The
 * table is neither thread safe nor reentrant (callbacks must not mutate the
 * same table). All operations are O(1) expected; no operation allocates except
 * insert, which allocates only when the table grows (or on the first insert). */

typedef struct LsMapTable {
    unsigned char* keys;      /* slot array, cap * key_size bytes   */
    unsigned char* values;    /* slot array, cap * value_size bytes */
    /* cap + 16 control bytes; the last 16 mirror the first 16. Empty slots
     * are 0x80, deleted slots 0xFE, occupied slots store H2 (0x00..0x7F). */
    unsigned char* ctrl;
    size_t len;
    size_t cap;               /* power of two (>= 16), 0 before first insert */
    size_t growth_left;       /* inserts before the next resize */
    size_t key_size;
    size_t value_size;
    uint64_t (*hash_key)(const void* key_bytes);
    bool (*eq_key)(const void* a, const void* b);
    void (*copy_key)(void* out, const void* in);
    void (*drop_key)(void* key_bytes);
    void (*copy_value)(void* out, const void* in);
    void (*drop_value)(void* value_bytes);
} LsMapTable;

typedef struct LsMapIter { const LsMapTable* table; size_t index; } LsMapIter;

/* Initialize an empty table. No memory is allocated until the first insert.
 * key_size/value_size are the C sizeof of the key/value types. */
void ls_map_init(LsMapTable* table, size_t key_size, size_t value_size,
                 uint64_t (*hash_key)(const void*),
                 bool (*eq_key)(const void*, const void*),
                 void (*copy_key)(void*, const void*),
                 void (*drop_key)(void*),
                 void (*copy_value)(void*, const void*),
                 void (*drop_value)(void*));
/* Release every stored key and value; keeps the slot arrays and capacity. */
void ls_map_clear(LsMapTable* table);
/* Insert a key/value pair, or overwrite the value of an existing key. Adopts
 * (moves) key and value. For an existing key the previous value is copied into
 * out_old_value when out_old_value is non-NULL, and the caller's duplicate key
 * is released. Returns true iff an old value was returned. */
bool ls_map_insert(LsMapTable* table, const void* key, const void* value,
                   void* out_old_value);
/* Borrowed pointer to the stored value bytes (valid until the next mutation of
 * the table); returns false when the key is absent. */
bool ls_map_get_ptr(const LsMapTable* table, const void* key,
                    void** out_value_ptr);
/* Remove an entry: moves its value into out_value (or releases the value when
 * out_value is NULL) and releases the stored key. Returns false when absent. */
bool ls_map_remove(LsMapTable* table, const void* key, void* out_value);
size_t ls_map_len(const LsMapTable* table);
bool ls_map_is_empty(const LsMapTable* table);
void ls_map_iter_init(LsMapIter* iter, const LsMapTable* table);
/* Write borrowed key/value pointers for the next occupied slot and return true,
 * or return false when the iteration is finished. */
bool ls_map_iter_next(LsMapIter* iter, const void** key_ptr,
                      const void** value_ptr);

/* Deterministic hashing/equality callbacks for the frozen key types of the
 * builtin HashMap (i64, u64, bool, String, char). The hash functions take a
 * pointer to the stored key bytes and return the full 64-bit hash; the map
 * itself derives the group index and H2 from it. ls_hash_bytes() is the
 * wyhash style byte hash with fixed seeds (no randomized state), so hashes are
 * deterministic for a given runtime version. ls_hash_string_slot() interprets
 * the slot as an LsString* handle (NULL hashes as 0) and hashes the string
 * bytes. The eq functions take two key byte pointers and compare the language
 * values; strings compare content (NULL only equals NULL). */
uint64_t ls_hash_bytes(const void* data, size_t len);
uint64_t ls_hash_i64(const void* slot);
uint64_t ls_hash_u64(const void* slot);
uint64_t ls_hash_bool(const void* slot);
uint64_t ls_hash_string_slot(const void* slot);
uint64_t ls_hash_char(const void* slot);
bool ls_eq_i64(const void* a, const void* b);
bool ls_eq_u64(const void* a, const void* b);
bool ls_eq_bool(const void* a, const void* b);
bool ls_eq_string_slot(const void* a, const void* b);
bool ls_eq_char(const void* a, const void* b);

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
/* A char: Display appends the raw UTF-8 encoding, Debug appends a quoted Rust
 * style character literal (for example 'a', '\n', '\u{1F600}'). */
LsString* ls_fmt_char(LsString* acc, uint32_t v);
LsString* ls_fmt_char_debug(LsString* acc, uint32_t v);
/* Shortest decimal representation that round-trips to the same bits.
 * f64 and f32 are formatted independently (0.1f32 prints as "0.1"). */
LsString* ls_fmt_f64(LsString* acc, double v);
LsString* ls_fmt_f32(LsString* acc, float v);
/* Debug ({:?}) formatting. */
LsString* ls_fmt_debug_string(LsString* acc, const LsString* v);
LsString* ls_fmt_f64_debug(LsString* acc, double v);
LsString* ls_fmt_f32_debug(LsString* acc, float v);

/* Format specifiers: precision, sign and padding.
 *
 * A format hole like {:<8} or {:+.3} is compiled into one of the precision
 * aware formatters below followed by ls_fmt_apply_width(). A negative
 * "precision" means "not specified": the ordinary shortest representation is
 * used. The "plus" flag prefixes a '+' to non-negative numbers (including
 * +inf and +NaN). */
LsString* ls_fmt_i64_signed(LsString* acc, int64_t v, bool plus);
LsString* ls_fmt_u64_signed(LsString* acc, uint64_t v, bool plus);
/* Fixed notation with exactly "precision" digits after the decimal point,
 * rounding half to even like printf("%.*f"), e.g. {:.2} on 1.005. */
LsString* ls_fmt_f64_prec(LsString* acc, double v, int32_t precision, bool plus);
LsString* ls_fmt_f32_prec(LsString* acc, float v, int32_t precision, bool plus);
/* At most "precision" Unicode scalar values of a string. */
LsString* ls_fmt_str_prec(LsString* acc, const LsString* v, int32_t precision);

/* Alignment values for ls_fmt_apply_width(). */
#define LS_ALIGN_LEFT 0
#define LS_ALIGN_RIGHT 1
#define LS_ALIGN_CENTER 2

/* Append "piece" to "acc" padded to a minimum display width of "width"
 * Unicode scalar values. "align" is one of the LS_ALIGN_* constants, "fill" is
 * a Unicode scalar value used as padding (space for the default), and
 * "zero_pad" inserts ASCII '0' after a leading sign instead of using the fill
 * (the 0 flag of format strings). "piece" is borrowed: the caller keeps
 * ownership and releases it. A width of at most 1, or any width that the piece
 * already reaches, appends the piece unchanged. */
LsString* ls_fmt_apply_width(LsString* acc, LsString* piece, int64_t width,
                             int32_t align, uint32_t fill, bool zero_pad);

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
/* Located variants used by generated code: validate that every argument is
 * valid UTF-8 and panic at the given location otherwise (the panic message is
 * "arguments contain invalid UTF-8"). */
LsVec_String ls_args_at(const char* file, int32_t line, int32_t col);

/* Read one line from stdin without the trailing newline ("\r\n" is
 * normalized). Returns tag 0 (None) on EOF, tag 1 (Some) with an owned string
 * otherwise. */
LsOpt_String ls_read_line(void);
/* Located variant used by generated code: panics with "stream did not contain
 * valid UTF-8" when stdin is not valid UTF-8. */
LsOpt_String ls_read_line_at(const char* file, int32_t line, int32_t col);

/* Read a whole file. The contents must be valid UTF-8: an invalid file yields
 * tag 1 (Err) with the message "stream did not contain valid UTF-8" instead of
 * panicking. On other errors the result has tag 1 (Err) and an owned OS error
 * message such as "No such file or directory (os error 2)". */
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
