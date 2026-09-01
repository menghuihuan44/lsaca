#include <stdio.h>
#include <stdarg.h>
#include "src/lsaca_vm.h"
#include "src/lsaca_disasm.h"
#include "src/lsaca_compiler.h"

ls_vm_t ls_vm;

static void reset_vm(ls_vm_t *vm) {
}

static void runtime_error(ls_vm_t *vm, const char *format, ...) {
    va_list args;
    va_start(args, format);
    vfprintf(stderr, format, args);
    va_end(args);
    fputs("\n", stderr);

    size_t line_dest = vm->ip - vm->chunk->codes - 1;
    size_t line = vm->chunk->lines[line_dest];
    fprintf(stderr, "[line %zu] in script\n", line);
    reset_vm(vm);
}

void ls_vm_init(ls_vm_t *vm) {
    reset_vm(vm);
}

void ls_vm_deinit(ls_vm_t *vm) {
}

static ls_vm_status run(ls_vm_t *vm) {
#define READ_BYTE() (*vm->ip++)

    for (;;) {
#if LS_DISASM_TRACE_EXECUTION != 0
        printf("          ");
        for (size_t i = 0; i < vm->chunk->constants.size; ++i) {
            printf("[ ");
            ls_any_print(vm->chunk->constants.values[i]);
            printf(" ]");
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
            case LS_OP_LOADNULL: {
                uint32_t a = LS_RA(inst);
                vm->regs[a] = LS_NULL_ANY;
                break;
            }
            case LS_OP_LOADFALSE: {
                uint32_t a = LS_RA(inst);
                vm->regs[a] = LS_BOOL_ANY(false);
                break;
            }
            case LS_OP_LOADTRUE: {
                uint32_t a = LS_RA(inst);
                vm->regs[a] = LS_BOOL_ANY(true);
                break;
            }
            case LS_OP_NEG: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Negation requires an i64 or f64 operand.");
                    runtime_error(vm, "Value's type is %s.", ls_any_type_name(vm->regs[b]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_I64_ANY(-LS_AS_I64(vm->regs[b])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_F64_ANY(-LS_AS_F64(vm->regs[b])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_NOT: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                if (!LS_IS_BOOL(vm->regs[b])) {
                    runtime_error(vm, "Logical NOT requires a boolean operand.");
                    runtime_error(vm, "Value's type is %s.", ls_any_type_name(vm->regs[b]));
                    return LS_VM_RUNTIME_ERROR;
                }

                vm->regs[a] = LS_BOOL_ANY(!LS_AS_BOOL(vm->regs[b]));
                break;
            }
            case LS_OP_BANG_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c]) &&
                (!LS_IS_NULL(vm->regs[b]) && !LS_IS_NULL(vm->regs[c]))) {
                    runtime_error(vm, "Both operands must be of the same type"
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (LS_IS_NULL(vm->regs[b]) || LS_IS_NULL(vm->regs[c])) {
                    vm->regs[a] = LS_BOOL_ANY(LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c]));
                    break;
                }
                if (!LS_IS_BOOL(vm->regs[b]) && !LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b]) && !LS_IS_STR(vm->regs[b])) {
                    runtime_error(vm, "Equality comparison requires boolean, i64, f64 or str operands."
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_BOOL: vm->regs[a] = LS_BOOL_ANY(LS_AS_BOOL(vm->regs[b]) != LS_AS_BOOL(vm->regs[c])); break;
                    case LS_ANY_I64: vm->regs[a] = LS_BOOL_ANY(LS_AS_I64(vm->regs[b]) != LS_AS_I64(vm->regs[c])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_BOOL_ANY(LS_AS_F64(vm->regs[b]) != LS_AS_F64(vm->regs[c])); break;
                    case LS_ANY_STR: vm->regs[a] = LS_BOOL_ANY(!ls_string_is_equal(LS_AS_STR(vm->regs[b]), LS_AS_STR(vm->regs[c]))); break;
                    default: break;
                }
                break;
            }
            case LS_OP_EQUAL_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c]) &&
                (!LS_IS_NULL(vm->regs[b]) && !LS_IS_NULL(vm->regs[c]))) {
                    runtime_error(vm, "Both operands must be of the same type"
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (LS_IS_NULL(vm->regs[b]) || LS_IS_NULL(vm->regs[c])) {
                    vm->regs[a] = LS_BOOL_ANY(LS_GET_TYPE(vm->regs[b]) == LS_GET_TYPE(vm->regs[c]));
                    break;
                }
                if (!LS_IS_BOOL(vm->regs[b]) && !LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b]) && !LS_IS_STR(vm->regs[b])) {
                    runtime_error(vm, "Equality comparison requires boolean, i64, f64 or str operands."
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_BOOL: vm->regs[a] = LS_BOOL_ANY(LS_AS_BOOL(vm->regs[b]) == LS_AS_BOOL(vm->regs[c])); break;
                    case LS_ANY_I64: vm->regs[a] = LS_BOOL_ANY(LS_AS_I64(vm->regs[b]) == LS_AS_I64(vm->regs[c])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_BOOL_ANY(LS_AS_F64(vm->regs[b]) == LS_AS_F64(vm->regs[c])); break;
                    case LS_ANY_STR: vm->regs[a] = LS_BOOL_ANY(ls_string_is_equal(LS_AS_STR(vm->regs[b]), LS_AS_STR(vm->regs[c]))); break;
                    default: break;
                }
                break;
            }
            case LS_OP_GREATER: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_BOOL_ANY(LS_AS_I64(vm->regs[b]) > LS_AS_I64(vm->regs[c])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_BOOL_ANY(LS_AS_F64(vm->regs[b]) > LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_GREATER_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_BOOL_ANY(LS_AS_I64(vm->regs[b]) >= LS_AS_I64(vm->regs[c])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_BOOL_ANY(LS_AS_F64(vm->regs[b]) >= LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_LESS: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_BOOL_ANY(LS_AS_I64(vm->regs[b]) < LS_AS_I64(vm->regs[c])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_BOOL_ANY(LS_AS_F64(vm->regs[b]) < LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_LESS_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_BOOL_ANY(LS_AS_I64(vm->regs[b]) <= LS_AS_I64(vm->regs[c])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_BOOL_ANY(LS_AS_F64(vm->regs[b]) <= LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_ADD: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Addition requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_I64_ANY((int64_t)((uint64_t)LS_AS_I64(vm->regs[b]) + (uint64_t)LS_AS_I64(vm->regs[c]))); break;
                    case LS_ANY_F64: vm->regs[a] = LS_F64_ANY(LS_AS_F64(vm->regs[b]) + LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_SUB: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Subtraction requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_I64_ANY((int64_t)((uint64_t)LS_AS_I64(vm->regs[b]) - (uint64_t)LS_AS_I64(vm->regs[c]))); break;
                    case LS_ANY_F64: vm->regs[a] = LS_F64_ANY(LS_AS_F64(vm->regs[b]) - LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_MUL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Multiplication requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_I64_ANY((int64_t)((uint64_t)LS_AS_I64(vm->regs[b]) * (uint64_t)LS_AS_I64(vm->regs[c]))); break;
                    case LS_ANY_F64: vm->regs[a] = LS_F64_ANY(LS_AS_F64(vm->regs[b]) * LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_DIV: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(vm->regs[b]) != LS_GET_TYPE(vm->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(vm->regs[b]) && !LS_IS_F64(vm->regs[b])) {
                    runtime_error(vm, "Division requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(vm->regs[b]), ls_any_type_name(vm->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (LS_IS_I64(vm->regs[c]) && LS_AS_I64(vm->regs[c]) == 0) {
                    runtime_error(vm, "Division by zero is not allowed for i64 operands.");
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(vm->regs[b])) {
                    case LS_ANY_I64: vm->regs[a] = LS_I64_ANY(LS_AS_I64(vm->regs[b]) / LS_AS_I64(vm->regs[c])); break;
                    case LS_ANY_F64: vm->regs[a] = LS_F64_ANY(LS_AS_F64(vm->regs[b]) / LS_AS_F64(vm->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_JUMP: {
                int32_t ax = LS_RsAx(inst);
                vm->ip += ax;
                break;
            }
            case LS_OP_JUMP_IF_FALSE: {
                uint32_t a = LS_RA(inst);
                int32_t bx = LS_RsBx(inst);
                if (!LS_IS_BOOL(vm->regs[a])) {
                    runtime_error(&ls_vm, "Condition must be a boolean expression.");
                    return LS_VM_RUNTIME_ERROR;
                }

                vm->ip += (!LS_AS_BOOL(vm->regs[a])) * bx;
                break;
            }
            case LS_OP_RETURN: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                
                /* It will add something in the future. */
                return LS_VM_OK;
            }
            /* temp */
            case LS_OP_PRINT: {
                uint32_t a = LS_RA(inst);

                ls_any_print(vm->regs[a]);
                printf("\n");
                break;
            }
            default:
                return LS_VM_RUNTIME_ERROR;
        }
    }

#undef READ_BYTE
}

ls_vm_status ls_vm_interpret(ls_vm_t *vm, const char *source) {
    ls_chunk_t chunk;
    ls_chunk_init(&chunk);

    if (!ls_compile(source, &chunk)) {
        ls_chunk_deinit(&chunk);
        return LS_VM_COMPILE_ERROR;
    }

    vm->chunk = &chunk;
    vm->ip = vm->chunk->codes;

    ls_vm_status result = run(vm);

    ls_chunk_deinit(&chunk);
    return result;
}