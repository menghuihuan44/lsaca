/* lsaca C17 runtime: hash map translation unit.
 *
 * Implements the by-value LsMapTable/LsMapIter container that generated code
 * uses for the builtin HashMap<K, V> type. The table is a plain byte container:
 * keys and values are stored with their C sizeof and six callbacks supplied by
 * the code generator provide the language-level hash/equality/copy/drop
 * semantics. Public declarations live in lsaca_rt.h; the shared private
 * contract lives in lsaca_rt_internal.h. The runtime never allocates the
 * generated wrapper, only the three slot arrays.
 *
 * Design (v1.6 frozen contract, see target/v16/DESIGN-MAP.md section 6):
 *   - SwissTable style open addressing with 16-slot groups and control bytes:
 *     0x00..=0x7F = occupied (H2 = hash & 0x7F), 0x80 = EMPTY (stops a probe),
 *     0xFE = DELETED (tombstone: probe continues, slot may be reused). The
 *     control array has cap + 16 bytes; the trailing 16 bytes mirror the first
 *     16 so a group load can never leave the allocation even when the first
 *     group is read at an offset (the probing code here only reads in-bounds
 *     16-byte groups, the mirror is kept for contract compliance).
 *   - The group of a key starts at (hash >> 7) & (cap / 16 - 1) and probes move
 *     forward group by group (wrapping) until a group with an EMPTY byte is
 *     found. Inside a group, the three byte masks (H2 matches, EMPTY, DELETED)
 *     are computed with portable SWAR arithmetic over two 64-bit loads
 *     (memcpy, no unaligned access, no strict-aliasing tricks, no SSE):
 *       zero_byte_mask(x) = ~(((x & 0x7F..) + 0x7F..) | x | 0x7F..)
 *     which is exact per byte. The design document sketches the classic
 *     borrow-based haszero mask; that variant can report a byte as zero due to
 *     borrow propagation from the byte below (for example 0x..0100 flags the
 *     0x01 byte), which would be an actual wrong slot for an EMPTY mask, so the
 *     exact mask above is used instead (still pure C17 SWAR).
 *   - Capacity is a power of two, at least 16, and 0 before the first insert.
 *     The load factor is 7/8: growth_left counts the remaining EMPTY insertions
 *     before a rehash is required. When an insert needs a new key and
 *     growth_left == 0, the table doubles when len >= cap - cap/8, otherwise it
 *     rehashes at the same capacity (which drops all tombstones). A slot that
 *     was EMPTY costs one growth_left unit; reusing a DELETED slot does not.
 *     Invariant: EMPTY slots == cap/8 + growth_left, so a probe always meets an
 *     EMPTY byte and terminates (no tombstone can fill the table completely).
 *   - Hashing is deterministic (fixed wyhash style secrets, no randomized
 *     state): 128-bit multiply mixing (__int128 when available, exact 32x32
 *     fallback otherwise) over little-endian 64-bit loads, with the length
 *     folded in. i64/u64/bool/char hash their representation bytes; String
 *     hashes data+len. Iteration visits slots in index order, deterministic for
 *     a given operation sequence and runtime version but an implementation
 *     detail, not a language contract.
 *
 * Ownership: an occupied slot owns its key byte blob and its value byte blob
 * (as language-level values, moved in by the caller).
 *   - insert adopts (moves) both incoming blobs. When the key already exists,
 *     the stored value is handed out through copy_value(out_old_value, slot) and
 *     then released, the incoming value is adopted, and the incoming duplicate
 *     key is released through drop_key (ownership of it was transferred to the
 *     map by the caller). The function returns true iff it wrote out_old_value.
 *   - remove moves the stored value bytes into out_value (or releases the stored
 *     value when out_value is NULL) and releases the stored key.
 *   - clear releases every stored key and value but keeps the capacity.
 *   - rehash/resize move occupied key/value bytes with memcpy; this is an
 *     ownership transfer and deliberately does not call any callback.
 * The generated wrapper's drop helper must call ls_map_clear() and then free()
 * keys, values and ctrl; the three pointers are NULL before the first insert.
 *
 * ls_hash_bytes/hashing and the table operations are neither thread safe nor
 * reentrant: callbacks must not mutate the table they are called for. Only
 * insert allocates (first insert and growth).
 */

#include "lsaca_rt_internal.h"

#if defined(_MSC_VER)
#include <intrin.h>
#endif

/* ====================================================================== */
/* Hashing and equality primitives                                        */
/* ====================================================================== */

/* Control byte values. An occupied slot stores H2 in 0x00..=0x7F. */
#define LS_MAP_CTRL_EMPTY ((unsigned char)0x80)
#define LS_MAP_CTRL_DELETED ((unsigned char)0xFE)
#define LS_MAP_H2_MASK ((uint64_t)0x7F)

/* Slots per control-byte group and the trailing mirror length. */
#define LS_MAP_GROUP_WIDTH ((size_t)16)

/* First capacity after the first insert; always a power of two. */
#define LS_MAP_INITIAL_CAPACITY ((size_t)16)

/* Load factor 7/8: cap / 8 slots stay EMPTY as a probe terminator. */
#define LS_MAP_LOAD_DENOMINATOR ((size_t)8)

/* Fixed wyhash style secrets. Deterministic (no address/time randomization);
 * changing them changes iteration order but not semantics. */
#define LS_MAP_SECRET0 ((uint64_t)0xa0761d6478bd642fULL)
#define LS_MAP_SECRET1 ((uint64_t)0xe7037ed1a0b428dbULL)
#define LS_MAP_SECRET2 ((uint64_t)0x8ebc6af09c88c6e3ULL)
#define LS_MAP_SECRET3 ((uint64_t)0x589965cc75374cc3ULL)

/* Byte-mask constants for the portable SWAR group compare. */
#define LS_MAP_ONES ((uint64_t)0x0101010101010101ULL)
#define LS_MAP_LOW7 ((uint64_t)0x7F7F7F7F7F7F7F7FULL)

/* 128-bit multiply folded to 64 bits (wyhash "mum"): xor of the high and low
 * halves. The 32x32 decomposition is exact and fully portable; both paths must
 * produce identical results so a fast-path build and a fallback build hash
 * identically. */
static uint64_t ls_map_mul128(uint64_t a, uint64_t b) {
#if defined(__SIZEOF_INT128__)
    __extension__ typedef unsigned __int128 ls_map_u128;
    ls_map_u128 product = (ls_map_u128)a * (ls_map_u128)b;
    return (uint64_t)(product >> 64) ^ (uint64_t)product;
#else
    uint64_t a_low = a & 0xFFFFFFFFu;
    uint64_t a_high = a >> 32;
    uint64_t b_low = b & 0xFFFFFFFFu;
    uint64_t b_high = b >> 32;
    uint64_t low_low = a_low * b_low;
    uint64_t low_high = a_low * b_high;
    uint64_t high_low = a_high * b_low;
    uint64_t high_high = a_high * b_high;
    uint64_t middle = (low_high & 0xFFFFFFFFu) + (high_low & 0xFFFFFFFFu);
    uint64_t low = low_low + (middle << 32);
    uint64_t carry = low < low_low ? 1u : 0u;
    uint64_t high = high_high + (low_high >> 32) + (high_low >> 32) +
                    (middle >> 32) + carry;
    return high ^ low;
#endif
}

/* Little-endian 64-bit load: deterministic independent of host endianness. */
static uint64_t ls_map_load64le(const unsigned char* bytes) {
    return (uint64_t)bytes[0] |
           ((uint64_t)bytes[1] << 8) |
           ((uint64_t)bytes[2] << 16) |
           ((uint64_t)bytes[3] << 24) |
           ((uint64_t)bytes[4] << 32) |
           ((uint64_t)bytes[5] << 40) |
           ((uint64_t)bytes[6] << 48) |
           ((uint64_t)bytes[7] << 56);
}

/* Load 1..7 trailing bytes as a little-endian value (len is never 0 or 8+). */
static uint64_t ls_map_load_tail(const unsigned char* bytes, size_t len) {
    uint64_t value = 0;
    size_t i;

    for (i = 0; i < len; i += 1) {
        value |= (uint64_t)bytes[i] << (8u * (unsigned)i);
    }
    return value;
}

/* wyhash style deterministic byte hash: 16-byte blocks of two 64-bit lanes,
 * then an 8-byte lane, then a 1..7 byte tail, with the length folded in. A NULL
 * data pointer with len == 0 (and, defensively, any NULL data pointer) hashes
 * as empty. */
uint64_t ls_hash_bytes(const void* data, size_t len) {
    const unsigned char* bytes = (const unsigned char*)data;
    uint64_t seed = LS_MAP_SECRET0 ^ (uint64_t)len;
    size_t index = 0;

    if (bytes != NULL) {
        while (len - index >= 16) {
            uint64_t first = ls_map_load64le(bytes + index);
            uint64_t second = ls_map_load64le(bytes + index + 8);
            seed = ls_map_mul128(first ^ LS_MAP_SECRET1, second ^ seed);
            index += 16;
        }
        if (len - index >= 8) {
            seed = ls_map_mul128(ls_map_load64le(bytes + index) ^ LS_MAP_SECRET2,
                                 seed ^ LS_MAP_SECRET3);
            index += 8;
        }
        if (len - index != 0) {
            uint64_t tail = ls_map_load_tail(bytes + index, len - index);
            seed = ls_map_mul128(tail ^ LS_MAP_SECRET2, seed ^ LS_MAP_SECRET3);
        }
    }
    seed ^= (uint64_t)len;
    seed = ls_map_mul128(seed ^ LS_MAP_SECRET1, LS_MAP_SECRET2);
    return seed;
}

/* Fixed-width scalar key fast paths. For a key of 1, 4 or 8 bytes the generic
 * ls_hash_bytes() reduces to exactly the arithmetic below (no 16-byte block, one
 * scalar lane, a single length mix), so these helpers produce BIT-IDENTICAL hash
 * values while skipping the generic block-loop/tail machinery. Bit-identical
 * values keep iteration order and every existing expected output unchanged. */
static uint64_t ls_map_hash_scalar(uint64_t value_le, size_t len) {
    uint64_t seed = LS_MAP_SECRET0 ^ (uint64_t)len;

    seed = ls_map_mul128(value_le ^ LS_MAP_SECRET2, seed ^ LS_MAP_SECRET3);
    seed ^= (uint64_t)len;
    return ls_map_mul128(seed ^ LS_MAP_SECRET1, LS_MAP_SECRET2);
}

static uint64_t ls_map_load32le(const unsigned char* bytes) {
    return (uint64_t)bytes[0] |
           ((uint64_t)bytes[1] << 8) |
           ((uint64_t)bytes[2] << 16) |
           ((uint64_t)bytes[3] << 24);
}

uint64_t ls_hash_i64(const void* slot) {
    int64_t value;

    memcpy(&value, slot, sizeof value);
    return ls_map_hash_scalar(ls_map_load64le((const unsigned char*)&value),
                              sizeof value);
}

uint64_t ls_hash_u64(const void* slot) {
    uint64_t value;

    memcpy(&value, slot, sizeof value);
    return ls_map_hash_scalar(ls_map_load64le((const unsigned char*)&value),
                              sizeof value);
}

uint64_t ls_hash_bool(const void* slot) {
    bool value;
    unsigned char byte;

    memcpy(&value, slot, sizeof value);
    byte = value ? (unsigned char)1 : (unsigned char)0;
    return ls_map_hash_scalar((uint64_t)byte, sizeof byte);
}

uint64_t ls_hash_string_slot(const void* slot) {
    LsString* value;

    memcpy(&value, slot, sizeof value);
    if (value == NULL) {
        return ls_hash_bytes(NULL, 0);
    }
    return ls_hash_bytes(ls_str_data(value), ls_str_len(value));
}

uint64_t ls_hash_char(const void* slot) {
    uint32_t value;

    memcpy(&value, slot, sizeof value);
    return ls_map_hash_scalar(ls_map_load32le((const unsigned char*)&value),
                              sizeof value);
}

bool ls_eq_i64(const void* a, const void* b) {
    int64_t left;
    int64_t right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    return left == right;
}

bool ls_eq_u64(const void* a, const void* b) {
    uint64_t left;
    uint64_t right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    return left == right;
}

bool ls_eq_bool(const void* a, const void* b) {
    bool left;
    bool right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    return left == right;
}

bool ls_eq_string_slot(const void* a, const void* b) {
    LsString* left;
    LsString* right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    if (left == NULL || right == NULL) {
        return left == right;
    }
    return ls_str_eq(left, right);
}

bool ls_eq_char(const void* a, const void* b) {
    uint32_t left;
    uint32_t right;

    memcpy(&left, a, sizeof left);
    memcpy(&right, b, sizeof right);
    return left == right;
}

/* ====================================================================== */
/* SWAR group masks                                                       */
/* ====================================================================== */

/* Broadcast a control byte to all eight bytes of a word. */
#define LS_MAP_BCAST(byte) (LS_MAP_ONES * (uint64_t)(byte))
#define LS_MAP_EMPTY_BCAST LS_MAP_BCAST(LS_MAP_CTRL_EMPTY)
#define LS_MAP_DELETED_BCAST LS_MAP_BCAST(LS_MAP_CTRL_DELETED)

/* Exact per-byte zero mask: the high bit of every byte that is zero is set.
 * The addition cannot carry between bytes ((x & 0x7F..) + 0x7F.. <= 0xFE per
 * byte), and the | x | 0x7F.. step keeps the high-bit result exact. A zero
 * byte of the input becomes 0x80 in the result (the high bit), all other bytes
 * become 0x00; see target/v16/DESIGN-MAP.md section 6.2. Kept as a macro so the
 * probe hot path never pays a function call for it. */
#define LS_MAP_ZERO_HIGH_BITS(word) \
    (~((((word) & LS_MAP_LOW7) + LS_MAP_LOW7) | (word) | LS_MAP_LOW7))

/* Pack the high bits of the eight bytes of a zero mask into 8 low bits. The
 * magic multiply folds byte i's high bit (bit 8*i+7) into bit i of the result;
 * this is exact (verified by target/v16/probes/migrator/map_swiss/mask_test.c). */
#define LS_MAP_PACK8(high_bits) \
    ((uint32_t)(((((high_bits) >> 7) & LS_MAP_ONES) * 0x0102040810204080ULL) >> 56))

/* High bit of every byte. */
#define LS_MAP_HIGH_BITS ((uint64_t)0x8080808080808080ULL)

/* Classic SWAR zero-byte test: the high bit of every zero byte is set. Unlike
 * the exact mask above it can also flag a 0x01 byte that follows a zero byte
 * (borrow propagation); that is harmless where it is used (H2 candidate
 * filtering: a false candidate costs one extra eq_key call that rejects it, and
 * the flagged slot is always an occupied slot because ctrl == h2^1 is always in
 * the occupied range). EMPTY/DELETED masks use the exact form. Fewer operations
 * and no 64-bit multiply, so the probe hot path uses this form for H2. */
#define LS_MAP_ZERO_FAST(word) \
    (((word) - LS_MAP_ONES) & ~(word) & LS_MAP_HIGH_BITS)

/* Prefetch the two cache lines that hold the key blobs of a group. The address
 * of a matching key depends on the control-byte mask (the slot index is only
 * known after the ctrl line arrives), so a probe would otherwise serialize two
 * dependent random cache misses: ctrl line, then key line. Issuing the key
 * prefetches as soon as the group index is known lets the key fetch overlap the
 * ctrl fetch; measured 1M-key present lookups: 69.6 ns/op without the hint,
 * 53.4 ns/op with it (interleaved probe, target/v16/probes/migrator/map_swiss/
 * interleaved_final.txt). GCC/Clang only; without the builtin the hint is a
 * no-op and the algorithm is unchanged. */
#if defined(__GNUC__) || defined(__clang__)
#define LS_MAP_PREFETCH_GROUP_KEYS(keys, group_base, key_size)                 \
    do {                                                                       \
        __builtin_prefetch((const void*)((keys) + (group_base) * (key_size))); \
        __builtin_prefetch((const void*)((keys) +                              \
            ((group_base) + LS_MAP_GROUP_WIDTH / 2) * (key_size)));            \
    } while (0)
#else
#define LS_MAP_PREFETCH_GROUP_KEYS(keys, group_base, key_size) \
    do {                                                       \
        (void)(keys);                                          \
        (void)(group_base);                                    \
        (void)(key_size);                                      \
    } while (0)
#endif

/* Index of the lowest set bit of a 64-bit mask; caller guarantees mask != 0. */
static unsigned ls_map_lowest_bit64(uint64_t mask) {
#if defined(__GNUC__) || defined(__clang__)
    return (unsigned)__builtin_ctzll(mask);
#elif defined(_MSC_VER)
    unsigned long index;

    _BitScanForward64(&index, (unsigned long long)mask);
    return (unsigned)index;
#else
    unsigned bit = 0;

    while (((mask >> bit) & 1ULL) == 0ULL) {
        bit += 1;
    }
    return bit;
#endif
}

/* 16-bit mask of the slots of a group whose control byte equals "byte".
 * Only used on cold paths (rehash); the probe inlines the same arithmetic. */
static uint32_t ls_map_group_mask(const unsigned char* group,
                                  unsigned char byte) {
    uint64_t first;
    uint64_t second;
    uint64_t broadcast = LS_MAP_BCAST(byte);

    memcpy(&first, group, sizeof first);
    memcpy(&second, group + 8, sizeof second);
    return LS_MAP_PACK8(LS_MAP_ZERO_HIGH_BITS(first ^ broadcast)) |
           (LS_MAP_PACK8(LS_MAP_ZERO_HIGH_BITS(second ^ broadcast)) << 8);
}

/* Index of the lowest set bit; the caller guarantees mask != 0. */
static unsigned ls_map_lowest_bit(uint32_t mask) {
#if defined(__GNUC__) || defined(__clang__)
    return (unsigned)__builtin_ctz((unsigned)mask);
#elif defined(_MSC_VER)
    unsigned long index;

    _BitScanForward(&index, (unsigned long)mask);
    return (unsigned)index;
#else
    unsigned bit = 0;

    while (((mask >> bit) & 1u) == 0u) {
        bit += 1;
    }
    return bit;
#endif
}

/* ====================================================================== */
/* Table internals                                                        */
/* ====================================================================== */

/* Allocate the three slot arrays for the given (power-of-two) capacity. The
 * control array is cap + 16 bytes and starts fully EMPTY (mirror included). On
 * allocation failure the process exits through the runtime OOM path. */
static void ls_map_allocate(LsMapTable* table, size_t capacity) {
    size_t key_bytes;
    size_t value_bytes;
    unsigned char* keys;
    unsigned char* values;
    unsigned char* ctrl;

    if (capacity < LS_MAP_INITIAL_CAPACITY ||
        (capacity & (capacity - 1)) != 0) {
        ls_rt_alloc_failure(SIZE_MAX);
    }
    if (table->key_size == 0 || table->value_size == 0 ||
        capacity > SIZE_MAX / table->key_size ||
        capacity > SIZE_MAX / table->value_size ||
        capacity > SIZE_MAX - LS_MAP_GROUP_WIDTH) {
        ls_rt_alloc_failure(SIZE_MAX);
    }
    key_bytes = capacity * table->key_size;
    value_bytes = capacity * table->value_size;
    keys = (unsigned char*)malloc(key_bytes);
    if (keys == NULL) {
        ls_rt_alloc_failure(key_bytes);
    }
    values = (unsigned char*)malloc(value_bytes);
    if (values == NULL) {
        ls_rt_alloc_failure(value_bytes);
    }
    ctrl = (unsigned char*)malloc(capacity + LS_MAP_GROUP_WIDTH);
    if (ctrl == NULL) {
        ls_rt_alloc_failure(capacity + LS_MAP_GROUP_WIDTH);
    }
    memset(ctrl, (int)LS_MAP_CTRL_EMPTY, capacity + LS_MAP_GROUP_WIDTH);
    table->keys = keys;
    table->values = values;
    table->ctrl = ctrl;
    table->cap = capacity;
    table->len = 0;
    table->growth_left = capacity - capacity / LS_MAP_LOAD_DENOMINATOR;
}

/* Shared probe core. Returns the slot holding the key (*found = true). When the
 * key is absent and want_insert_slot is true, returns the slot the key should be
 * inserted into: the earliest tombstone met on the scan path, else the first
 * EMPTY slot of the terminating group. Inside a group, slots are scanned in
 * index order, so a tombstone only wins over an EMPTY byte when it lies before
 * that EMPTY byte - that is what keeps tombstone slots reusable (a removed key
 * is normally found again as a tombstone at the head of its own probe path).
 * When want_insert_slot is false (lookups and removals) an absent key returns
 * SIZE_MAX and the DELETED mask is never computed.
 *
 * The three control-byte masks of a group are computed inline with SWAR over two
 * 64-bit memcpy loads; the H2 broadcast is built once per probe. The H2 mask is
 * computed first so a successful lookup (the common case: ~7/8 of occupied
 * groups terminate immediately) never pays for the EMPTY or DELETED mask.
 *
 * The table maintains EMPTY slots == cap/8 + growth_left, so a probe always
 * terminates; SIZE_MAX on the insert path is only returned for a defensively
 * detected corrupt table (the caller treats it as an allocation failure). */
static size_t ls_map_probe_impl(const LsMapTable* table, const void* key,
                                uint64_t hash, bool want_insert_slot,
                                bool* found) {
    size_t groups = table->cap / LS_MAP_GROUP_WIDTH;
    size_t group_mask = groups - 1;
    size_t group_index = (size_t)((hash >> 7) & (uint64_t)group_mask);
    unsigned char h2 = (unsigned char)(hash & LS_MAP_H2_MASK);
    uint64_t h2_broadcast = LS_MAP_BCAST(h2);
    size_t first_tombstone = SIZE_MAX;
    size_t step;
    bool (*eq_key)(const void*, const void*) = table->eq_key;
    const unsigned char* keys = table->keys;
    size_t key_size = table->key_size;

    for (step = 0; step < groups; step += 1) {
        size_t group_base = group_index * LS_MAP_GROUP_WIDTH;
        const unsigned char* group;
        uint64_t first;
        uint64_t second;
        uint64_t match_h2;
        uint64_t match_empty;
        uint64_t match_deleted;
        unsigned offset;

        LS_MAP_PREFETCH_GROUP_KEYS(keys, group_base, key_size);
        group = table->ctrl + group_base;
        memcpy(&first, group, sizeof first);
        memcpy(&second, group + 8, sizeof second);
        /* H2 candidates are scanned before any EMPTY/DELETED mask is built, so
         * a successful lookup (the common case) only pays for the H2 masks. */
        match_h2 = LS_MAP_ZERO_FAST(first ^ h2_broadcast);
        while (match_h2 != 0) {
            size_t slot;

            offset = ls_map_lowest_bit64(match_h2);
            slot = group_base + (size_t)(offset >> 3);
            if (eq_key(keys + slot * key_size, key)) {
                *found = true;
                return slot;
            }
            match_h2 &= ~((uint64_t)0x80 << (offset & 56u));
        }
        match_h2 = LS_MAP_ZERO_FAST(second ^ h2_broadcast);
        while (match_h2 != 0) {
            size_t slot;

            offset = ls_map_lowest_bit64(match_h2);
            slot = group_base + 8 + (size_t)(offset >> 3);
            if (eq_key(keys + slot * key_size, key)) {
                *found = true;
                return slot;
            }
            match_h2 &= ~((uint64_t)0x80 << (offset & 56u));
        }
        match_empty = LS_MAP_ZERO_HIGH_BITS(first ^ LS_MAP_EMPTY_BCAST);
        if (match_empty != 0) {
            /* The terminating EMPTY byte is in the first half of the group. */
            if (!want_insert_slot) {
                *found = false;
                return SIZE_MAX;
            }
            *found = false;
            offset = (unsigned)(ls_map_lowest_bit64(match_empty) >> 3);
            if (first_tombstone != SIZE_MAX) {
                return first_tombstone;
            }
            match_deleted = LS_MAP_ZERO_HIGH_BITS(first ^ LS_MAP_DELETED_BCAST);
            if (match_deleted != 0 &&
                (ls_map_lowest_bit64(match_deleted) >> 3) < offset) {
                return group_base + (size_t)(ls_map_lowest_bit64(match_deleted) >> 3);
            }
            return group_base + (size_t)offset;
        }
        match_empty = LS_MAP_ZERO_HIGH_BITS(second ^ LS_MAP_EMPTY_BCAST);
        if (match_empty != 0) {
            /* The terminating EMPTY byte is in the second half of the group. */
            if (!want_insert_slot) {
                *found = false;
                return SIZE_MAX;
            }
            *found = false;
            offset = 8u + (unsigned)(ls_map_lowest_bit64(match_empty) >> 3);
            if (first_tombstone != SIZE_MAX) {
                return first_tombstone;
            }
            match_deleted = LS_MAP_ZERO_HIGH_BITS(first ^ LS_MAP_DELETED_BCAST);
            if (match_deleted != 0) {
                return group_base + (size_t)(ls_map_lowest_bit64(match_deleted) >> 3);
            }
            match_deleted = LS_MAP_ZERO_HIGH_BITS(second ^ LS_MAP_DELETED_BCAST);
            if (match_deleted != 0) {
                unsigned deleted_offset =
                    (unsigned)(ls_map_lowest_bit64(match_deleted) >> 3);

                if (deleted_offset < offset - 8u) {
                    return group_base + 8 + (size_t)deleted_offset;
                }
            }
            return group_base + (size_t)offset;
        }
        /* No EMPTY byte in this group: remember the earliest tombstone. */
        if (first_tombstone == SIZE_MAX) {
            match_deleted = LS_MAP_ZERO_HIGH_BITS(first ^ LS_MAP_DELETED_BCAST);
            if (match_deleted != 0) {
                first_tombstone =
                    group_base + (size_t)(ls_map_lowest_bit64(match_deleted) >> 3);
            } else {
                match_deleted =
                    LS_MAP_ZERO_HIGH_BITS(second ^ LS_MAP_DELETED_BCAST);
                if (match_deleted != 0) {
                    first_tombstone =
                        group_base + 8 +
                        (size_t)(ls_map_lowest_bit64(match_deleted) >> 3);
                }
            }
        }
        group_index = (group_index + 1) & group_mask;
    }
    *found = false;
    return first_tombstone;
}

/* Probe used by inserts: absent keys return their insertion slot. */
static size_t ls_map_probe(const LsMapTable* table, const void* key,
                           uint64_t hash, bool* found) {
    return ls_map_probe_impl(table, key, hash, true, found);
}

/* Probe used by lookups/removals: absent keys return SIZE_MAX. */
static size_t ls_map_find(const LsMapTable* table, const void* key,
                          uint64_t hash, bool* found) {
    return ls_map_probe_impl(table, key, hash, false, found);
}

/* First EMPTY slot on the probe path of hash in a table that has no tombstones
 * on that path (used when rehashing into fresh arrays). */
static size_t ls_map_find_empty_slot(const LsMapTable* table, uint64_t hash) {
    size_t groups = table->cap / LS_MAP_GROUP_WIDTH;
    size_t group_mask = groups - 1;
    size_t group_index = (size_t)((hash >> 7) & (uint64_t)group_mask);
    size_t step;

    for (step = 0; step < groups; step += 1) {
        const unsigned char* group =
            table->ctrl + group_index * LS_MAP_GROUP_WIDTH;
        uint32_t match_empty = ls_map_group_mask(group, LS_MAP_CTRL_EMPTY);

        if (match_empty != 0) {
            return group_index * LS_MAP_GROUP_WIDTH +
                   (size_t)ls_map_lowest_bit(match_empty);
        }
        group_index = (group_index + 1) & group_mask;
    }
    ls_rt_alloc_failure(SIZE_MAX);
}

/* Rebuild the table into new arrays of the given (power-of-two) capacity.
 * Occupied key/value bytes are moved with memcpy (ownership transfer, no
 * callbacks); tombstones are eliminated by construction. */
static void ls_map_rehash(LsMapTable* table, size_t capacity) {
    unsigned char* old_keys = table->keys;
    unsigned char* old_values = table->values;
    unsigned char* old_ctrl = table->ctrl;
    size_t old_cap = table->cap;
    size_t key_size = table->key_size;
    size_t value_size = table->value_size;
    size_t moved = 0;
    size_t index;

    ls_map_allocate(table, capacity);
    for (index = 0; index < old_cap; index += 1) {
        uint64_t hash;
        size_t slot;
        unsigned char h2;

        if (old_ctrl[index] >= LS_MAP_CTRL_EMPTY) {
            continue;
        }
        hash = table->hash_key(old_keys + index * key_size);
        slot = ls_map_find_empty_slot(table, hash);
        memcpy(table->keys + slot * key_size,
               old_keys + index * key_size, key_size);
        memcpy(table->values + slot * value_size,
               old_values + index * value_size, value_size);
        h2 = (unsigned char)(hash & LS_MAP_H2_MASK);
        table->ctrl[slot] = h2;
        if (slot < LS_MAP_GROUP_WIDTH) {
            table->ctrl[table->cap + slot] = h2;
        }
        moved += 1;
    }
    table->len = moved;
    table->growth_left =
        capacity - capacity / LS_MAP_LOAD_DENOMINATOR - moved;
    memcpy(table->ctrl + capacity, table->ctrl, LS_MAP_GROUP_WIDTH);
    free(old_keys);
    free(old_values);
    free(old_ctrl);
}

/* ====================================================================== */
/* Table operations                                                       */
/* ====================================================================== */

void ls_map_init(LsMapTable* table, size_t key_size, size_t value_size,
                 uint64_t (*hash_key)(const void*),
                 bool (*eq_key)(const void*, const void*),
                 void (*copy_key)(void*, const void*),
                 void (*drop_key)(void*),
                 void (*copy_value)(void*, const void*),
                 void (*drop_value)(void*)) {
    memset(table, 0, sizeof *table);
    table->key_size = key_size;
    table->value_size = value_size;
    table->hash_key = hash_key;
    table->eq_key = eq_key;
    table->copy_key = copy_key;
    table->drop_key = drop_key;
    table->copy_value = copy_value;
    table->drop_value = drop_value;
}

bool ls_map_insert(LsMapTable* table, const void* key, const void* value,
                   void* out_old_value) {
    uint64_t hash;
    bool found = false;
    size_t slot;

    if (table->cap == 0) {
        ls_map_allocate(table, LS_MAP_INITIAL_CAPACITY);
    }
    hash = table->hash_key(key);
    slot = ls_map_probe(table, key, hash, &found);
    if (!found && table->growth_left == 0) {
        size_t capacity = table->cap;

        if (table->len >= capacity - capacity / LS_MAP_LOAD_DENOMINATOR) {
            if (capacity > SIZE_MAX / 2) {
                ls_rt_alloc_failure(SIZE_MAX);
            }
            ls_map_rehash(table, capacity * 2);
        } else {
            ls_map_rehash(table, capacity);
        }
        slot = ls_map_probe(table, key, hash, &found);
    }
    if (found) {
        unsigned char* old_value = table->values + slot * table->value_size;

        if (out_old_value != NULL) {
            table->copy_value(out_old_value, old_value);
        }
        table->drop_value(old_value);
        memcpy(old_value, value, table->value_size);
        /* The caller transferred ownership of the key bytes to the map, but the
         * public signature takes them as const (insert never mutates the key).
         * The drop callback only releases the owned bytes, so the const
         * qualifier is dropped here on purpose. */
        table->drop_key((void*)key);
        return out_old_value != NULL;
    }
    if (slot == SIZE_MAX) {
        /* Unreachable while the EMPTY slot invariant holds; defensive only. */
        ls_rt_alloc_failure(SIZE_MAX);
    }
    {
        bool was_empty = table->ctrl[slot] == LS_MAP_CTRL_EMPTY;
        unsigned char h2 = (unsigned char)(hash & LS_MAP_H2_MASK);

        memcpy(table->keys + slot * table->key_size, key, table->key_size);
        memcpy(table->values + slot * table->value_size, value,
               table->value_size);
        table->ctrl[slot] = h2;
        if (slot < LS_MAP_GROUP_WIDTH) {
            table->ctrl[table->cap + slot] = h2;
        }
        table->len += 1;
        if (was_empty) {
            table->growth_left -= 1;
        }
    }
    return false;
}

bool ls_map_get_ptr(const LsMapTable* table, const void* key,
                    void** out_value_ptr) {
    bool found = false;
    size_t slot;

    if (table->cap == 0) {
        return false;
    }
    slot = ls_map_find(table, key, table->hash_key(key), &found);
    if (!found) {
        return false;
    }
    if (out_value_ptr != NULL) {
        *out_value_ptr = (void*)(table->values + slot * table->value_size);
    }
    return true;
}

bool ls_map_remove(LsMapTable* table, const void* key, void* out_value) {
    bool found = false;
    size_t slot;

    if (table->cap == 0) {
        return false;
    }
    slot = ls_map_find(table, key, table->hash_key(key), &found);
    if (!found) {
        return false;
    }
    if (out_value != NULL) {
        memcpy(out_value, table->values + slot * table->value_size,
               table->value_size);
    } else {
        table->drop_value(table->values + slot * table->value_size);
    }
    table->drop_key(table->keys + slot * table->key_size);
    table->ctrl[slot] = LS_MAP_CTRL_DELETED;
    if (slot < LS_MAP_GROUP_WIDTH) {
        table->ctrl[table->cap + slot] = LS_MAP_CTRL_DELETED;
    }
    table->len -= 1;
    return true;
}

size_t ls_map_len(const LsMapTable* table) {
    return table->len;
}

bool ls_map_is_empty(const LsMapTable* table) {
    return table->len == 0;
}

void ls_map_clear(LsMapTable* table) {
    size_t index;

    if (table->cap == 0) {
        table->len = 0;
        return;
    }
    for (index = 0; index < table->cap; index += 1) {
        if (table->ctrl[index] < LS_MAP_CTRL_EMPTY) {
            table->drop_key(table->keys + index * table->key_size);
            table->drop_value(table->values + index * table->value_size);
        }
    }
    memset(table->ctrl, (int)LS_MAP_CTRL_EMPTY,
           table->cap + LS_MAP_GROUP_WIDTH);
    table->len = 0;
    table->growth_left =
        table->cap - table->cap / LS_MAP_LOAD_DENOMINATOR;
}

void ls_map_iter_init(LsMapIter* iter, const LsMapTable* table) {
    iter->table = table;
    iter->index = 0;
}

bool ls_map_iter_next(LsMapIter* iter, const void** key_ptr,
                      const void** value_ptr) {
    const LsMapTable* table = iter->table;
    size_t index = iter->index;

    while (table != NULL && index < table->cap) {
        if (table->ctrl[index] < LS_MAP_CTRL_EMPTY) {
            if (key_ptr != NULL) {
                *key_ptr = table->keys + index * table->key_size;
            }
            if (value_ptr != NULL) {
                *value_ptr = table->values + index * table->value_size;
            }
            iter->index = index + 1;
            return true;
        }
        index += 1;
    }
    if (table != NULL) {
        iter->index = index;
    }
    return false;
}
