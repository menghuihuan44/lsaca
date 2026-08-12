#include "src/lsaca_chunk.h"
#include "src/lsaca_disasm.h"
#include "src/base/lsaca_gc.h"

int main(int argc, const char *argv[]) {
    ls_gc_init();

    ls_chunk_t chunk;
    ls_chunk_init(&chunk);

    size_t k = ls_chunk_add_constant(&chunk, 42.0);
    ls_chunk_append(&chunk, LS_ABx(LS_OP_LOADK, 0, k), 1);

    ls_chunk_append(&chunk, LS_OP_RETURN, 2);
    ls_disasm_chunk(&chunk, "test chunk");
    ls_chunk_deinit(&chunk);

    ls_gc_deinit();
    return 0;
}