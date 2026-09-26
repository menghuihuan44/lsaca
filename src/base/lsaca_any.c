#include <stdio.h>
#include "src/base/lsaca_any.h"
#include "src/base/lsaca_object.h"

static const char *any_type_names[] = {
    "null",
    "boolean",
    "i64",
    "f64",
    "str",
    "func",
};

const char *ls_any_type_name(ls_any_t value) {
    return any_type_names[LS_GET_TYPE(value)];
}

void ls_any_print(ls_any_t value) {
    switch (LS_GET_TYPE(value)) {
        case LS_ANY_NULL: {
            printf("null");
            break;
        }
        case LS_ANY_BOOL: {
            printf(LS_AS_BOOL(value) ? "true" : "false");
            break;
        }
        case LS_ANY_I64: {
            printf("%"PRId64, LS_AS_I64(value));
            break;
        }
        case LS_ANY_F64: {
            printf("%f", LS_AS_F64(value));
            break;
        }
        case LS_ANY_STR: {
            ls_string_t *str = LS_AS_STR(value);
            printf("%s", str->c_str);
            break;
        }
        case LS_ANY_FUNC: {
            ls_function_t *func = LS_AS_FUNC(value);
            if (func->name != NULL) {
                printf("<func %s>", func->name->c_str);
            } else {
                printf("<func script>");
            }
            break;
        }
        default: {
            fprintf(stderr, "Can not print it. Its type is %s\n", any_type_names[LS_GET_TYPE(value)]);
        }
    }
}