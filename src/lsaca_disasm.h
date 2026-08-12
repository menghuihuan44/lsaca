#ifndef LSACA_DISASM_H
#define LSACA_DISASM_H

#include "src/lsaca_chunk.h"

#ifdef __cplusplus
extern "C" {
#endif

void ls_disasm_chunk(ls_chunk_t *chunk, const char *name);
size_t ls_disasm_inst(ls_chunk_t *chunk, size_t offset);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_DISASM_H */