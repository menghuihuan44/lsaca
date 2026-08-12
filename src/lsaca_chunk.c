#include <stdlib.h>
#include "src/lsaca_chunk.h"
#include "src/base/lsaca_gc.h"

void ls_chunk_init(ls_chunk_t *chunk) {
    if (!chunk) return;

    chunk->codes = NULL;
    chunk->count = 0;
    chunk->capacity = 0;
    chunk->lines = NULL;
    ls_array_init(&chunk->constants);
}

void ls_chunk_deinit(ls_chunk_t *chunk) {
    if (!chunk) return;

    ls_array_deinit(&chunk->constants);
    ls_gc_remove_root(&ls_gc, (void **)&chunk->codes);
    chunk->count = 0;
    chunk->capacity = 0;
    ls_gc_remove_root(&ls_gc, (void **)&chunk->lines);
}

void ls_chunk_append(ls_chunk_t *chunk, ls_inst_t code, size_t line) {
    if (!chunk) return;
    
    if (chunk->capacity < chunk->count + 1) {
        ls_inst_t *old_codes = chunk->codes;
        size_t *old_lines = chunk->lines;
        size_t old_capa = chunk->capacity == 0 ? 1 : chunk->capacity;

        chunk->codes = (ls_inst_t *)ls_gc_realloc(&ls_gc, chunk->codes, old_capa << 1);
        if (!chunk->codes) {
            chunk->codes = old_codes;
            return;
        }
        chunk->lines = (size_t *)ls_gc_realloc(&ls_gc, chunk->lines, old_capa << 1);
        if (!chunk->lines) {
            chunk->lines = old_lines;
            return;
        }

        if (chunk->capacity == 0) {
            ls_gc_add_root(&ls_gc, (void **)&chunk->codes);
            ls_gc_add_root(&ls_gc, (void **)&chunk->lines);
        }
        chunk->capacity = old_capa << 1;
    }

    chunk->codes[chunk->count] = code;
    chunk->lines[chunk->count] = line;
    ++chunk->count;
}

size_t ls_chunk_add_constant(ls_chunk_t *chunk, ls_any_t value) {
    if (!chunk) return (size_t)-1;

    size_t index = chunk->constants.count;
    ls_array_append(&chunk->constants, value);
    return index;
}