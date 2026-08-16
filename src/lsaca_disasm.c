#include <stdio.h>
#include "src/lsaca_disasm.h"
#include "src/base/lsaca_any.h"

static const char *opcode_names[] = {
    "MOVE",
    "LOADK",
    "LOADNULL",
    "LOADFALSE",
    "LOADTRUE",
    "NEG",
    "NOT",
    "BANG_EQUAL",
    "EQUAL_EQUAL",
    "GREATER",
    "GREATER_EQUAL",
    "LESS",
    "LESS_EQUAL",
    "ADD",
    "SUB",
    "MUL",
    "DIV",
    "RETURN",
};

static size_t disasm_ABC(ls_chunk_t *chunk, size_t offset) {
    ls_inst_t inst = chunk->codes[offset];
    uint32_t a = LS_RA(inst);
    uint32_t b = LS_RB(inst);
    uint32_t c = LS_RC(inst);

    printf("%04zu: %-8s R%"PRIu32" R%"PRIu32" R%"PRIu32"\n", offset, opcode_names[LS_OP(inst)], a, b, c);
    return offset + 1;
}

static size_t disasm_ABx(ls_chunk_t *chunk, size_t offset) {
    ls_inst_t inst = chunk->codes[offset];
    uint32_t a = LS_RA(inst);
    uint32_t bx = LS_RBx(inst);

    printf("%04zu: %-8s R%"PRIu32" K%"PRIu32" '", offset, opcode_names[LS_OP(inst)], a, bx);
    ls_any_print(chunk->constants.values[bx]);
    printf("'\n");
    return offset + 1;
}

static size_t disasm_Ax(ls_chunk_t *chunk, size_t offset) {
    ls_inst_t inst = chunk->codes[offset];
    uint32_t ax = LS_RAx(inst);

    printf("%04zu: %-8s K%"PRIu32" '", offset, opcode_names[LS_OP(inst)], ax);
    ls_any_print(chunk->constants.values[ax]);
    printf("'\n");
    return offset + 1;
}

void ls_disasm_chunk(ls_chunk_t *chunk, const char *name) {
    printf("== %s ==\n", name);

    for (size_t offset = 0; offset < chunk->count;) {
        offset = ls_disasm_inst(chunk, offset);
    }
}

size_t ls_disasm_inst(ls_chunk_t *chunk, size_t offset) {
    if (offset > 0 && chunk->lines[offset] == chunk->lines[offset - 1]) {
        printf("|    ");
    } else {
        printf("%-4zu ", chunk->lines[offset]);
    }

    ls_inst_t inst = chunk->codes[offset];
    switch (LS_OP(inst)) {
        case LS_OP_MOVE:
            return disasm_ABC(chunk, offset);
        case LS_OP_LOADK:
            return disasm_ABx(chunk, offset);
        case LS_OP_LOADNULL:
            return disasm_ABC(chunk, offset);
        case LS_OP_LOADFALSE:
            return disasm_ABC(chunk, offset);
        case LS_OP_LOADTRUE:
            return disasm_ABC(chunk, offset);
        case LS_OP_NEG:
            return disasm_ABC(chunk, offset);
        case LS_OP_NOT:
            return disasm_ABC(chunk, offset);
        case LS_OP_BANG_EQUAL:
            return disasm_ABC(chunk, offset);
        case LS_OP_EQUAL_EQUAL:
            return disasm_ABC(chunk, offset);
        case LS_OP_GREATER:
            return disasm_ABC(chunk, offset);
        case LS_OP_GREATER_EQUAL:
            return disasm_ABC(chunk, offset);
        case LS_OP_LESS:
            return disasm_ABC(chunk, offset);
        case LS_OP_LESS_EQUAL:
            return disasm_ABC(chunk, offset);
        case LS_OP_ADD:
            return disasm_ABC(chunk, offset);
        case LS_OP_SUB:
            return disasm_ABC(chunk, offset);
        case LS_OP_MUL:
            return disasm_ABC(chunk, offset);
        case LS_OP_DIV:
            return disasm_ABC(chunk, offset);
        case LS_OP_RETURN:
            return disasm_ABC(chunk, offset);
        default:
            printf("%04zu: Unknown opcode %"PRIu32"\n", offset, LS_OP(inst));
            return offset + 1;
    }
}