#include <stdlib.h>
#include <string.h>
#include "src/base/lsaca_string.h"
#include "src/base/lsaca_gc.h"

#define ALIGN2(x) (((x) + 2 - 1) & ~(2 - 1))

static size_t utf8_char_len(unsigned char first_byte) {
    if ((first_byte & 0x80) == 0) return 1;       // 0xxxxxxx
    if ((first_byte & 0xE0) == 0xC0) return 2;    // 110xxxxx
    if ((first_byte & 0xF0) == 0xE0) return 3;    // 1110xxxx
    if ((first_byte & 0xF8) == 0xF0) return 4;    // 11110xxx
    return 0;
}

static uint32_t utf8_decode(const unsigned char *bytes, size_t len) {
    switch (len) {
        case 1: return bytes[0];
        case 2: return ((bytes[0] & 0x1F) << 6)  | (bytes[1] & 0x3F);
        case 3: return ((bytes[0] & 0x0F) << 12) | ((bytes[1] & 0x3F) << 6) | (bytes[2] & 0x3F);
        case 4: return ((bytes[0] & 0x07) << 18) | ((bytes[1] & 0x3F) << 12) | ((bytes[2] & 0x3F) << 6) | (bytes[3] & 0x3F);
        default: return 0;
    }
}

static size_t utf8_encode(uint32_t cp, unsigned char out[4]) {
    if (cp <= 0x7F) {
        out[0] = cp;
        return 1;
    } else if (cp <= 0x7FF) {
        out[0] = 0xC0 | (cp >> 6);
        out[1] = 0x80 | (cp & 0x3F);
        return 2;
    } else if (cp <= 0xFFFF) {
        out[0] = 0xE0 | (cp >> 12);
        out[1] = 0x80 | ((cp >> 6) & 0x3F);
        out[2] = 0x80 | (cp & 0x3F);
        return 3;
    } else if (cp <= 0x10FFFF) {
        out[0] = 0xF0 | (cp >> 18);
        out[1] = 0x80 | ((cp >> 12) & 0x3F);
        out[2] = 0x80 | ((cp >> 6) & 0x3F);
        out[3] = 0x80 | (cp & 0x3F);
        return 4;
    }
    return 0;
}

ls_string_t *ls_string_create(const char *c_str) {
    if (!c_str) return NULL;

    return ls_string_create_with_len(c_str, strlen(c_str));
}

ls_string_t *ls_string_create_with_len(const char *c_str, size_t len) {
    if (!c_str) return NULL;

    size_t capa = ALIGN2(len + 1);
    ls_string_t *str = (ls_string_t *)ls_gc_realloc(&ls_gc, NULL, sizeof(ls_string_t) + capa * sizeof(char));
    if (!str) {
        return NULL;
    }
    str->size = len;
    str->capacity = capa;

    memcpy(str->c_str, c_str, len);
    str->c_str[len] = '\0';

    return str;
}

void ls_string_destroy(ls_string_t *str) {
    if (!str) return;

    str->size = 0;
    str->capacity = 0;
}

bool ls_string_is_equal(const ls_string_t *a, const ls_string_t *b) {
    if (!a || !b) return a == b;
    if (a->size != b->size) return false;
    return memcmp(a->c_str, b->c_str, a->size) == 0;
}