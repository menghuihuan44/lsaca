#ifndef LSACA_STRING_H
#define LSACA_STRING_H

#include "src/lsaca_common.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    size_t size;
    size_t capacity;
    char c_str[];
} ls_string_t;

ls_string_t *ls_string_create(const char *c_str);
ls_string_t *ls_string_create_with_len(const char *c_str, size_t len);
void ls_string_destroy(ls_string_t *str);

bool ls_string_is_equal(const ls_string_t *a, const ls_string_t *b);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_STRING_H */