#ifndef LSACA_VM_H
#define LSACA_VM_H

#include "src/lsaca_chunk.h"
#include "src/base/lsaca_object.h"

#ifdef __cplusplus
extern "C" {
#endif

#define LS_MAX_FRAMES 64
#define LS_MAX_STACK (LS_MAX_FRAMES * LS_UINT8_COUNT)

typedef struct {
    ls_function_t *function;
    ls_inst_t *ip;
    ls_any_t *regs;
} ls_call_frame_t;

typedef struct {
    ls_any_t stack[LS_MAX_STACK];
    ls_call_frame_t frames[LS_MAX_FRAMES];
    size_t frame_count;
} ls_vm_t;

typedef enum {
    LS_VM_OK,
    LS_VM_COMPILE_ERROR,
    LS_VM_RUNTIME_ERROR,
} ls_vm_status;

extern ls_vm_t ls_vm;

void ls_vm_init(ls_vm_t *vm);
void ls_vm_deinit(ls_vm_t *vm);

ls_vm_status ls_vm_interpret(ls_vm_t *vm, const char *source);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_VM_H */