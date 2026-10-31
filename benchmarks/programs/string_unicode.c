// string_unicode: exercise UTF-8 string handling over a deterministic
// mixed-ASCII/Unicode corpus: build 120,000 scalars one at a time, fold the
// scalars from iteration and random access, apply full-Unicode uppercase,
// slice at character boundaries, extend, pop and fold again.
// The checksum is <char_count>,<byte_len>,<folded scalar checksum>.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MOD 1000000007LL

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} Buf;

static void fatal(const char *what) {
    fprintf(stderr, "string_unicode: %s\n", what);
    exit(1);
}

static void buf_reserve(Buf *buf, size_t extra) {
    if (buf->len + extra <= buf->cap) {
        return;
    }
    size_t cap = buf->cap != 0 ? buf->cap : 64;
    while (cap < buf->len + extra) {
        cap *= 2;
    }
    char *data = (char *)realloc(buf->data, cap);
    if (data == NULL) {
        fatal("out of memory");
    }
    buf->data = data;
    buf->cap = cap;
}

static void buf_append(Buf *buf, const char *bytes, size_t count) {
    buf_reserve(buf, count);
    memcpy(buf->data + buf->len, bytes, count);
    buf->len += count;
}

static void buf_push_byte(Buf *buf, char byte) {
    buf_reserve(buf, 1);
    buf->data[buf->len++] = byte;
}

static size_t utf8_encode(uint32_t cp, char out[4]) {
    if (cp < 0x80) {
        out[0] = (char)cp;
        return 1;
    }
    if (cp < 0x800) {
        out[0] = (char)(0xC0 | (cp >> 6));
        out[1] = (char)(0x80 | (cp & 0x3F));
        return 2;
    }
    if (cp < 0x10000) {
        out[0] = (char)(0xE0 | (cp >> 12));
        out[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[2] = (char)(0x80 | (cp & 0x3F));
        return 3;
    }
    out[0] = (char)(0xF0 | (cp >> 18));
    out[1] = (char)(0x80 | ((cp >> 12) & 0x3F));
    out[2] = (char)(0x80 | ((cp >> 6) & 0x3F));
    out[3] = (char)(0x80 | (cp & 0x3F));
    return 4;
}

static size_t utf8_decode(const char *bytes, uint32_t *cp) {
    unsigned char first = (unsigned char)bytes[0];
    if (first < 0x80) {
        *cp = first;
        return 1;
    }
    if ((first & 0xE0) == 0xC0) {
        *cp = ((uint32_t)(first & 0x1F) << 6) | ((unsigned char)bytes[1] & 0x3F);
        return 2;
    }
    if ((first & 0xF0) == 0xE0) {
        *cp = ((uint32_t)(first & 0x0F) << 12) |
              ((uint32_t)((unsigned char)bytes[1] & 0x3F) << 6) |
              ((unsigned char)bytes[2] & 0x3F);
        return 3;
    }
    *cp = ((uint32_t)(first & 0x07) << 18) |
          ((uint32_t)((unsigned char)bytes[1] & 0x3F) << 12) |
          ((uint32_t)((unsigned char)bytes[2] & 0x3F) << 6) |
          ((unsigned char)bytes[3] & 0x3F);
    return 4;
}

static int is_continuation(unsigned char byte) {
    return (byte & 0xC0) == 0x80;
}

static const uint32_t ALPHABET[] = {
    'a', 0x00E9, 0x00F1, 0x00FC, 0x00F6, 0x00DF, 0xFB01, 0x00E0, 0x00E7,
    0x03B1, 0x03B2, 0x03B3, 0x03B4, 0x0430, 0x0431, 0x0432, 0x4F60, 0x597D,
    0x4E16, 0x754C, 0x1F600, 'k', 'q', 'w',
};
#define ALPHABET_LEN ((int64_t)(sizeof(ALPHABET) / sizeof(ALPHABET[0])))

static int upper_map(uint32_t cp, uint32_t out[2]) {
    if (cp >= (uint32_t)'a' && cp <= (uint32_t)'z') {
        out[0] = cp - 32;
        return 1;
    }
    switch (cp) {
    case 0x00E9: out[0] = 0x00C9; return 1;
    case 0x00F1: out[0] = 0x00D1; return 1;
    case 0x00FC: out[0] = 0x00DC; return 1;
    case 0x00F6: out[0] = 0x00D6; return 1;
    case 0x00DF: out[0] = 'S'; out[1] = 'S'; return 2;
    case 0xFB01: out[0] = 'F'; out[1] = 'I'; return 2;
    case 0x00E0: out[0] = 0x00C0; return 1;
    case 0x00E7: out[0] = 0x00C7; return 1;
    case 0x03B1: out[0] = 0x0391; return 1;
    case 0x03B2: out[0] = 0x0392; return 1;
    case 0x03B3: out[0] = 0x0393; return 1;
    case 0x03B4: out[0] = 0x0394; return 1;
    case 0x0430: out[0] = 0x0410; return 1;
    case 0x0431: out[0] = 0x0411; return 1;
    case 0x0432: out[0] = 0x0412; return 1;
    default: out[0] = cp; return 1;
    }
}

static int64_t mix(int64_t fold, int64_t value) {
    return (fold * 131 + value) % MOD;
}

int main(void) {
    const int64_t n = 120000;
    Buf base = {0};
    for (int64_t i = 0; i < n; i++) {
        char encoded[4];
        size_t used = utf8_encode(ALPHABET[i % ALPHABET_LEN], encoded);
        buf_append(&base, encoded, used);
    }
    int64_t fold = 0;
    {
        size_t at = 0;
        while (at < base.len) {
            uint32_t cp;
            at += utf8_decode(base.data + at, &cp);
            fold = mix(fold, (int64_t)cp);
        }
    }
    {
        int64_t probe = 0;
        int64_t char_index = 0;
        size_t at = 0;
        while (at < base.len && probe < 1000) {
            uint32_t cp;
            at += utf8_decode(base.data + at, &cp);
            if (char_index == probe * 7) {
                fold = mix(fold, (int64_t)cp);
                probe += 1;
            }
            char_index += 1;
        }
    }
    Buf upper = {0};
    {
        size_t at = 0;
        while (at < base.len) {
            uint32_t cp;
            uint32_t mapped[2];
            int count;
            at += utf8_decode(base.data + at, &cp);
            count = upper_map(cp, mapped);
            for (int u = 0; u < count; u++) {
                char encoded[4];
                size_t used = utf8_encode(mapped[u], encoded);
                buf_append(&upper, encoded, used);
            }
        }
    }
    {
        size_t at = 0;
        while (at < upper.len) {
            uint32_t cp;
            at += utf8_decode(upper.data + at, &cp);
            fold = mix(fold, (int64_t)cp);
        }
    }
    size_t mid = base.len / 2;
    while (mid > 0 && is_continuation((unsigned char)base.data[mid])) {
        mid -= 1;
    }
    size_t end = mid + 5000;
    while (end < base.len && is_continuation((unsigned char)base.data[end])) {
        end -= 1;
    }
    Buf built = {0};
    buf_append(&built, base.data + mid, end - mid);
    buf_push_byte(&built, '!');
    {
        size_t at = 0;
        int64_t appended = 0;
        while (at < upper.len && appended < 1000) {
            char encoded[4];
            uint32_t cp;
            size_t used = utf8_decode(upper.data + at, &cp);
            (void)cp;
            memcpy(encoded, upper.data + at, used);
            at += used;
            buf_append(&built, encoded, used);
            appended += 1;
        }
    }
    {
        size_t last = built.len - 1;
        uint32_t popped;
        while (last > 0 && is_continuation((unsigned char)built.data[last])) {
            last -= 1;
        }
        (void)utf8_decode(built.data + last, &popped);
        fold = mix(fold, (int64_t)popped);
        built.len = last;
    }
    buf_append(&built, "done", 4);
    {
        int64_t char_count = 0;
        size_t at = 0;
        while (at < built.len) {
            uint32_t cp;
            at += utf8_decode(built.data + at, &cp);
            fold = mix(fold, (int64_t)cp);
            char_count += 1;
        }
        printf("checksum=%lld,%lld,%lld\n", (long long)char_count,
               (long long)built.len, (long long)fold);
    }
    free(base.data);
    free(upper.data);
    free(built.data);
    return 0;
}
