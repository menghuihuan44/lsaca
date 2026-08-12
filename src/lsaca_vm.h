#ifndef LSACA_VM_H
#define LSACA_VM_H

#include "src/lsaca_chunk.h"

#ifdef __cplusplus
extern "C" {
#endif

#define LS_MAX_REGS 256

typedef struct {
    ls_chunk_t *chunk;
    ls_inst_t *ip;

    ls_any_t regs[LS_MAX_REGS];
    ls_any_t *regs_end;
} ls_vm_t;

typedef enum {
    LS_VM_OK,
    LS_VM_COMPILE_ERROR,
    LS_VM_RUNTIME_ERROR,
} ls_vm_status;

extern ls_vm_t ls_vm;

void ls_vm_init(ls_vm_t *vm);
void ls_vm_deinit(ls_vm_t *vm);

ls_vm_status ls_vm_interpret(ls_vm_t *vm, ls_chunk_t *chunk);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_VM_H */