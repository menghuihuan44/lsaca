#ifndef LSACA_CHUNK_H
#define LSACA_CHUNK_H

#include "src/lsaca_common.h"
#include "src/base/lsaca_array.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef uint32_t ls_inst_t;

/*
 * ls_inst_t format:
 *  LS_ABC: [C(8)][B(8)][A(8)][OP(8)]
 *  LS_ABx: [Bx(16)][A(8)][OP(8)]
 *  LS_Ax:  [Ax(24)][OP(8)]
 */

#define LS_SIZE_OP 8

#define LS_SIZE_A 8
#define LS_SIZE_B 8
#define LS_SIZE_C 8

#define LS_SIZE_Bx 16
#define LS_SIZE_Ax 24

#define LS_POS_A (LS_SIZE_OP)
#define LS_POS_B (LS_POS_A + LS_SIZE_A)
#define LS_POS_C (LS_POS_B + LS_SIZE_B)

#define LS_POS_Bx (LS_POS_A + LS_SIZE_A)
#define LS_POS_Ax (LS_SIZE_OP)

#define LS_OP(i) ((i) & ((1 << LS_SIZE_OP) - 1))

#define LS_RA(i) (((i) >> LS_POS_A) & ((1 << LS_SIZE_A) - 1))
#define LS_RB(i) (((i) >> LS_POS_B) & ((1 << LS_SIZE_B) - 1))
#define LS_RC(i) (((i) >> LS_POS_C) & ((1 << LS_SIZE_C) - 1))

#define LS_RBx(i) (((i) >> LS_POS_Bx) & ((1 << LS_SIZE_Bx) - 1))
#define LS_RAx(i) (((i) >> LS_POS_Ax) & ((1 << LS_SIZE_Ax) - 1))

#define LS_ABC(op, a, b, c) \
    ((ls_inst_t)(op) | ((ls_inst_t)(a) << LS_POS_A) | ((ls_inst_t)(b) << LS_POS_B) | ((ls_inst_t)(c) << LS_POS_C))
#define LS_ABx(op, a, bx) \
    ((ls_inst_t)(op) | ((ls_inst_t)(a) << LS_POS_A) | ((ls_inst_t)(bx) << LS_POS_Bx))
#define LS_Ax(op, ax) \
    ((ls_inst_t)(op) | ((ls_inst_t)(ax) << LS_POS_Ax))

typedef enum {
    LS_OP_MOVE, /* R(A) := R(B) */
    LS_OP_LOADK, /* R(A) := K(Bx) */
    LS_OP_RETURN, /* return R(A) */
} ls_opcode;

typedef struct {
    ls_inst_t *codes;
    size_t count;
    size_t capacity;
    size_t *lines;
    ls_array_t constants;
} ls_chunk_t;

void ls_chunk_init(ls_chunk_t *chunk);
void ls_chunk_deinit(ls_chunk_t *chunk);

void ls_chunk_append(ls_chunk_t *chunk, ls_inst_t code, size_t line);
size_t ls_chunk_add_constant(ls_chunk_t *chunk, ls_any_t value);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_CHUNK_H*/