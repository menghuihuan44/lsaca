#ifndef LSACA_OBJECT_H
#define LSACA_OBJECT_H

#include "src/lsaca_common.h"
#include "src/lsaca_chunk.h"
#include "src/base/lsaca_string.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ls_function_t {
    ls_chunk_t chunk;
    ls_string_t *name;
    size_t arity;
} ls_function_t;

ls_function_t *ls_function_create(void);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_OBJECT_H */