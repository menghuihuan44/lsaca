#ifndef LSACA_ANY_H
#define LSACA_ANY_H

#include "src/lsaca_common.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ls_array_t ls_array_t;

typedef double ls_any_t;

void ls_any_print(ls_any_t value);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_ANY_H */