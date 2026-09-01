#ifndef LSACA_ANY_H
#define LSACA_ANY_H

#include "src/lsaca_common.h"
#include "src/base/lsaca_string.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    LS_ANY_NULL,
    LS_ANY_BOOL,
    LS_ANY_I64,
    LS_ANY_F64,
    LS_ANY_STR,
} ls_any_type;

typedef struct {
    union {
        bool boolean;
        int64_t i64num;
        double f64num;
        ls_string_t *str;
    } as;
    ls_any_type type;
} ls_any_t;

#define LS_GET_TYPE(value) ((value).type)

#define LS_IS_NULL(value) ((value).type == LS_ANY_NULL)
#define LS_IS_BOOL(value) ((value).type == LS_ANY_BOOL)
#define LS_IS_I64(value) ((value).type == LS_ANY_I64)
#define LS_IS_F64(value) ((value).type == LS_ANY_F64)
#define LS_IS_STR(value) ((value).type == LS_ANY_STR)

#define LS_AS_BOOL(value) ((value).as.boolean)
#define LS_AS_I64(value) ((value).as.i64num)
#define LS_AS_F64(value) ((value).as.f64num)
#define LS_AS_STR(value) ((value).as.str)

#define LS_NULL_ANY ((ls_any_t){.as.boolean = false, .type = LS_ANY_NULL})
#define LS_BOOL_ANY(value) ((ls_any_t){.as.boolean = (value), .type = LS_ANY_BOOL})
#define LS_I64_ANY(value) ((ls_any_t){.as.i64num = (value), .type = LS_ANY_I64})
#define LS_F64_ANY(value) ((ls_any_t){.as.f64num = (value), .type = LS_ANY_F64})
#define LS_STR_ANY(value) ((ls_any_t){.as.str = (value), .type = LS_ANY_STR})

const char *ls_any_type_name(ls_any_t value);
void ls_any_print(ls_any_t value);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_ANY_H */