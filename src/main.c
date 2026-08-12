#include "src/lsaca_chunk.h"
#include "src/lsaca_disasm.h"
#include "src/lsaca_vm.h"
#include "src/base/lsaca_gc.h"

int main(int argc, const char *argv[]) {
    ls_gc_init(&ls_gc);
    ls_vm_init(&ls_vm);

    ls_chunk_t chunk;
    ls_chunk_init(&chunk);

    size_t k = ls_chunk_add_constant(&chunk, 42.0);
    ls_chunk_append(&chunk, LS_ABx(LS_OP_LOADK, 0, k), 1);
    ls_chunk_append(&chunk, LS_ABC(LS_OP_NEG, 0, 0, 0), 2);

    size_t k2 = ls_chunk_add_constant(&chunk, 3.14);
    ls_chunk_append(&chunk, LS_ABx(LS_OP_LOADK, 1, k2), 3);
    ls_chunk_append(&chunk, LS_ABC(LS_OP_ADD, 0, 0, 1), 3);
    ls_chunk_append(&chunk, LS_ABC(LS_OP_SUB, 0, 0, 1), 3);
    ls_chunk_append(&chunk, LS_ABC(LS_OP_MUL, 0, 0, 1), 3);
    ls_chunk_append(&chunk, LS_ABC(LS_OP_DIV, 0, 0, 1), 3);
    ls_chunk_append(&chunk, LS_ABC(LS_OP_MOVE, 1, 0, 0), 3);

    ls_chunk_append(&chunk, LS_OP_RETURN, 3);
    ls_disasm_chunk(&chunk, "test chunk");
    ls_vm_interpret(&ls_vm, &chunk);
    ls_chunk_deinit(&chunk);

    ls_vm_deinit(&ls_vm);
    ls_gc_deinit(&ls_gc);
    return 0;
}