#include <stdio.h>
#include "src/lsaca_vm.h"
#include "src/lsaca_disasm.h"

ls_vm_t ls_vm;

static void reset_regs(ls_vm_t *vm) {
    vm->regs_end = vm->regs;
}

void ls_vm_init(ls_vm_t *vm) {
    reset_regs(vm);
}

void ls_vm_deinit(ls_vm_t *vm) {
}

static ls_vm_status run(ls_vm_t *vm) {
#define READ_BYTE() (*vm->ip++)

#define BINARY_OP(op, inst) \
    do { \
        uint32_t a = LS_RA(inst); \
        uint32_t b = LS_RB(inst); \
        uint32_t c = LS_RC(inst); \
        \
        vm->regs[a] = vm->regs[b] op vm->regs[c]; \
    } while(0) \

    for (;;) {
#ifdef LS_DISASM_TRACE_EXECUTION
        printf("== REGISTERS ==\n");
        for (size_t i = 0; i < LS_MAX_REGS; ++i) {
            printf("[R%zu: ", i);
            ls_any_print(vm->regs[i]);
            printf("] ");
        }
        printf("\n");
        ls_disasm_inst(vm->chunk, (size_t)(vm->ip - vm->chunk->codes));
#endif

        ls_inst_t inst = READ_BYTE();
        switch (LS_OP(inst)) {
            case LS_OP_MOVE: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                vm->regs[a] = vm->regs[b];
                break;
            }
            case LS_OP_LOADK: {
                uint32_t a = LS_RA(inst);
                uint32_t bx = LS_RBx(inst);
                vm->regs[a] = vm->chunk->constants.values[bx];
                break;
            }
            case LS_OP_NEG: {
                uint32_t a = LS_RA(inst);
                vm->regs[a] = -vm->regs[a];
                break;
            }
            case LS_OP_ADD: {
                BINARY_OP(+, inst);
                break;
            }
            case LS_OP_SUB: {
                BINARY_OP(-, inst);
                break;
            }
            case LS_OP_MUL: {
                BINARY_OP(*, inst);
                break;
            }
            case LS_OP_DIV: {
                BINARY_OP(/, inst);
            }
            case LS_OP_RETURN: {
                return LS_VM_OK;
            }
            default:
                return LS_VM_RUNTIME_ERROR;
        }
    }

#undef READ_BYTE
#undef BINARY_OP
}

ls_vm_status ls_vm_interpret(ls_vm_t *vm, ls_chunk_t *chunk) {
    vm->chunk = chunk;
    vm->ip = vm->chunk->codes;
    return run(vm);
}