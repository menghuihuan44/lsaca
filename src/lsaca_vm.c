#include <stdio.h>
#include <stdarg.h>
#include "src/lsaca_vm.h"
#include "src/lsaca_disasm.h"
#include "src/lsaca_compiler.h"

ls_vm_t ls_vm;

static void reset_vm(ls_vm_t *vm) {
    vm->frame_count = 0;
}

static void runtime_error(ls_vm_t *vm, const char *format, ...) {
    va_list args;
    va_start(args, format);
    vfprintf(stderr, format, args);
    va_end(args);
    fputs("\n", stderr);

    for (size_t i = vm->frame_count - 1; i != SIZE_MAX; --i) {
        ls_call_frame_t *frame = &vm->frames[i];
        ls_function_t *function = frame->function;
        size_t instruction = frame->ip - function->chunk.codes - 1;
        fprintf(stderr, "[line %zu] in ", function->chunk.lines[instruction]);
        if (function->name == NULL) {
            fprintf(stderr, "script\n");
        } else {
            fprintf(stderr, "%s()\n", function->name->c_str);
        }
    }
    reset_vm(vm);
}

void ls_vm_init(ls_vm_t *vm) {
    reset_vm(vm);
}

void ls_vm_deinit(ls_vm_t *vm) {
}

static ls_vm_status run(ls_vm_t *vm) {
    ls_call_frame_t *frame = &vm->frames[vm->frame_count - 1];

#define READ_INST() (*frame->ip++)

    for (;;) {
#if LS_DISASM_TRACE_EXECUTION != 0
        printf("          ");
        for (size_t i = 0; i < frame->function->chunk.constants.size; ++i) {
            printf("[ ");
            ls_any_print(frame->function->chunk.constants.values[i]);
            printf(" ]");
        }
        printf("\n");
        ls_disasm_inst(&frame->function->chunk, (size_t)(frame->ip - frame->function->chunk.codes));
#endif

        ls_inst_t inst = READ_INST();
        switch (LS_OP(inst)) {
            case LS_OP_MOVE: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                frame->regs[a] = frame->regs[b];
                break;
            }
            case LS_OP_LOADK: {
                uint32_t a = LS_RA(inst);
                uint32_t bx = LS_RBx(inst);
                frame->regs[a] = frame->function->chunk.constants.values[bx];
                break;
            }
            case LS_OP_LOADNULL: {
                uint32_t a = LS_RA(inst);
                frame->regs[a] = LS_NULL_ANY;
                break;
            }
            case LS_OP_LOADFALSE: {
                uint32_t a = LS_RA(inst);
                frame->regs[a] = LS_BOOL_ANY(false);
                break;
            }
            case LS_OP_LOADTRUE: {
                uint32_t a = LS_RA(inst);
                frame->regs[a] = LS_BOOL_ANY(true);
                break;
            }
            case LS_OP_NEG: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Negation requires an i64 or f64 operand.");
                    runtime_error(vm, "Value's type is %s.", ls_any_type_name(frame->regs[b]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_I64_ANY(-LS_AS_I64(frame->regs[b])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_F64_ANY(-LS_AS_F64(frame->regs[b])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_NOT: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                if (!LS_IS_BOOL(frame->regs[b])) {
                    runtime_error(vm, "Logical NOT requires a boolean operand.");
                    runtime_error(vm, "Value's type is %s.", ls_any_type_name(frame->regs[b]));
                    return LS_VM_RUNTIME_ERROR;
                }

                frame->regs[a] = LS_BOOL_ANY(!LS_AS_BOOL(frame->regs[b]));
                break;
            }
            case LS_OP_BANG_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c]) &&
                (!LS_IS_NULL(frame->regs[b]) && !LS_IS_NULL(frame->regs[c]))) {
                    runtime_error(vm, "Both operands must be of the same type"
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (LS_IS_NULL(frame->regs[b]) || LS_IS_NULL(frame->regs[c])) {
                    frame->regs[a] = LS_BOOL_ANY(LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c]));
                    break;
                }
                if (!LS_IS_BOOL(frame->regs[b]) && !LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b]) && !LS_IS_STR(frame->regs[b])) {
                    runtime_error(vm, "Equality comparison requires boolean, i64, f64 or str operands."
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_BOOL: frame->regs[a] = LS_BOOL_ANY(LS_AS_BOOL(frame->regs[b]) != LS_AS_BOOL(frame->regs[c])); break;
                    case LS_ANY_I64: frame->regs[a] = LS_BOOL_ANY(LS_AS_I64(frame->regs[b]) != LS_AS_I64(frame->regs[c])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_BOOL_ANY(LS_AS_F64(frame->regs[b]) != LS_AS_F64(frame->regs[c])); break;
                    case LS_ANY_STR: frame->regs[a] = LS_BOOL_ANY(!ls_string_is_equal(LS_AS_STR(frame->regs[b]), LS_AS_STR(frame->regs[c]))); break;
                    default: break;
                }
                break;
            }
            case LS_OP_EQUAL_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c]) &&
                (!LS_IS_NULL(frame->regs[b]) && !LS_IS_NULL(frame->regs[c]))) {
                    runtime_error(vm, "Both operands must be of the same type"
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (LS_IS_NULL(frame->regs[b]) || LS_IS_NULL(frame->regs[c])) {
                    frame->regs[a] = LS_BOOL_ANY(LS_GET_TYPE(frame->regs[b]) == LS_GET_TYPE(frame->regs[c]));
                    break;
                }
                if (!LS_IS_BOOL(frame->regs[b]) && !LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b]) && !LS_IS_STR(frame->regs[b])) {
                    runtime_error(vm, "Equality comparison requires boolean, i64, f64 or str operands."
                        "(except for null, and if one of the operands is null, the other can be any type).");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_BOOL: frame->regs[a] = LS_BOOL_ANY(LS_AS_BOOL(frame->regs[b]) == LS_AS_BOOL(frame->regs[c])); break;
                    case LS_ANY_I64: frame->regs[a] = LS_BOOL_ANY(LS_AS_I64(frame->regs[b]) == LS_AS_I64(frame->regs[c])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_BOOL_ANY(LS_AS_F64(frame->regs[b]) == LS_AS_F64(frame->regs[c])); break;
                    case LS_ANY_STR: frame->regs[a] = LS_BOOL_ANY(ls_string_is_equal(LS_AS_STR(frame->regs[b]), LS_AS_STR(frame->regs[c]))); break;
                    default: break;
                }
                break;
            }
            case LS_OP_GREATER: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_BOOL_ANY(LS_AS_I64(frame->regs[b]) > LS_AS_I64(frame->regs[c])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_BOOL_ANY(LS_AS_F64(frame->regs[b]) > LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_GREATER_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_BOOL_ANY(LS_AS_I64(frame->regs[b]) >= LS_AS_I64(frame->regs[c])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_BOOL_ANY(LS_AS_F64(frame->regs[b]) >= LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_LESS: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_BOOL_ANY(LS_AS_I64(frame->regs[b]) < LS_AS_I64(frame->regs[c])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_BOOL_ANY(LS_AS_F64(frame->regs[b]) < LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_LESS_EQUAL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Comparison requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                
                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_BOOL_ANY(LS_AS_I64(frame->regs[b]) <= LS_AS_I64(frame->regs[c])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_BOOL_ANY(LS_AS_F64(frame->regs[b]) <= LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_ADD: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Addition requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_I64_ANY((int64_t)((uint64_t)LS_AS_I64(frame->regs[b]) + (uint64_t)LS_AS_I64(frame->regs[c]))); break;
                    case LS_ANY_F64: frame->regs[a] = LS_F64_ANY(LS_AS_F64(frame->regs[b]) + LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_SUB: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Subtraction requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_I64_ANY((int64_t)((uint64_t)LS_AS_I64(frame->regs[b]) - (uint64_t)LS_AS_I64(frame->regs[c]))); break;
                    case LS_ANY_F64: frame->regs[a] = LS_F64_ANY(LS_AS_F64(frame->regs[b]) - LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_MUL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Multiplication requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_I64_ANY((int64_t)((uint64_t)LS_AS_I64(frame->regs[b]) * (uint64_t)LS_AS_I64(frame->regs[c]))); break;
                    case LS_ANY_F64: frame->regs[a] = LS_F64_ANY(LS_AS_F64(frame->regs[b]) * LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_DIV: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                uint32_t c = LS_RC(inst);
                if (LS_GET_TYPE(frame->regs[b]) != LS_GET_TYPE(frame->regs[c])) {
                    runtime_error(vm, "Both operands must be of the same type.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (!LS_IS_I64(frame->regs[b]) && !LS_IS_F64(frame->regs[b])) {
                    runtime_error(vm, "Division requires i64 or f64 operands.");
                    runtime_error(vm, "Left value's type is %s and right value's type is %s.",
                        ls_any_type_name(frame->regs[b]), ls_any_type_name(frame->regs[c]));
                    return LS_VM_RUNTIME_ERROR;
                }
                if (LS_IS_I64(frame->regs[c]) && LS_AS_I64(frame->regs[c]) == 0) {
                    runtime_error(vm, "Division by zero is not allowed for i64 operands.");
                    return LS_VM_RUNTIME_ERROR;
                }

                switch (LS_GET_TYPE(frame->regs[b])) {
                    case LS_ANY_I64: frame->regs[a] = LS_I64_ANY(LS_AS_I64(frame->regs[b]) / LS_AS_I64(frame->regs[c])); break;
                    case LS_ANY_F64: frame->regs[a] = LS_F64_ANY(LS_AS_F64(frame->regs[b]) / LS_AS_F64(frame->regs[c])); break;
                    default: break;
                }
                break;
            }
            case LS_OP_JUMP: {
                int32_t ax = LS_RsAx(inst);
                frame->ip += ax;
                break;
            }
            case LS_OP_JUMP_IF_FALSE: {
                uint32_t a = LS_RA(inst);
                int32_t bx = LS_RsBx(inst);
                if (!LS_IS_BOOL(frame->regs[a])) {
                    runtime_error(vm, "Condition must be a boolean expression.");
                    return LS_VM_RUNTIME_ERROR;
                }

                frame->ip += (!LS_AS_BOOL(frame->regs[a])) * bx;
                break;
            }
            case LS_OP_CALL: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);

                if (!LS_IS_FUNC(frame->regs[a])) {
                    runtime_error(vm, "Can only call functions.");
                    return LS_VM_RUNTIME_ERROR;
                }
                if (vm->frame_count == LS_MAX_FRAMES) {
                    runtime_error(vm, "Stack overflow.");
                    return LS_VM_RUNTIME_ERROR;
                }
                ls_function_t *function = LS_AS_FUNC(frame->regs[a]);
                if (function->arity != b) {
                    runtime_error(vm, "Expected %d arguments but got %d.", function->arity, b);
                    return LS_VM_RUNTIME_ERROR;
                }

                ls_any_t *new_regs = &frame->regs[a + 1];
                frame = &vm->frames[vm->frame_count++];
                frame->function = function;
                frame->ip = function->chunk.codes;
                frame->regs = new_regs;
                break;
            }
            case LS_OP_RETURN: {
                uint32_t a = LS_RA(inst);
                uint32_t b = LS_RB(inst);
                
                ls_any_t ret_val = frame->regs[a];
                if (--vm->frame_count == 0) {
                    return LS_VM_OK;
                }

                frame = &vm->frames[vm->frame_count - 1];
                ls_inst_t caller_inst = *(frame->ip - 1);
                uint32_t caller_a = LS_RA(caller_inst);
                frame->regs[caller_a] = ret_val;
                break;
            }
            /* temp */
            case LS_OP_PRINT: {
                uint32_t a = LS_RA(inst);

                ls_any_print(frame->regs[a]);
                printf("\n");
                break;
            }
            default:
                return LS_VM_RUNTIME_ERROR;
        }
    }

#undef READ_INST
}

ls_vm_status ls_vm_interpret(ls_vm_t *vm, const char *source) {
    ls_function_t *function = ls_compile(source);
    if (!function) {
        return LS_VM_COMPILE_ERROR;
    }

    vm->frame_count = 0;
    ls_call_frame_t *frame = &vm->frames[vm->frame_count++];
    frame->function = function;
    frame->ip = function->chunk.codes;
    frame->regs = vm->stack;

    return run(vm);
}