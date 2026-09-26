#ifndef LSACA_COMPILER_H
#define LSACA_COMPILER_H

#include "src/lsaca_vm.h"

#ifdef __cplusplus
extern "C" {
#endif

ls_function_t *ls_compile(const char *source);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_COMPILER_H */