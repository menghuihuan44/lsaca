#ifndef LSACA_COMPILER_H
#define LSACA_COMPILER_H

#include "src/lsaca_vm.h"

#ifdef __cplusplus
extern "C" {
#endif

bool ls_compile(const char *source, ls_chunk_t *chunk);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_COMPILER_H */