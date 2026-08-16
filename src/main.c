#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "src/lsaca_chunk.h"
#include "src/lsaca_disasm.h"
#include "src/lsaca_vm.h"
#include "src/base/lsaca_gc.h"

static void repl(void) {
    char line[1024];
    for (;;) {
        printf("> ");
        if (!fgets(line, sizeof(line), stdin)) {
            printf("\n");
            break;
        }

        ls_vm_interpret(&ls_vm, line);
    }
}

static char *read_file(const char *path) {
    FILE *file = fopen(path, "rb");
    if (!file) {
        fprintf(stderr, "Failed to open file \"%s\".\n", path);
        exit(EXIT_FAILURE);
    }

    fseek(file, 0L, SEEK_END);
    size_t file_size = ftell(file);
    rewind(file);

    char *buffer = (char *)malloc(file_size + 1);
    if (!buffer) {
        fprintf(stderr, "Failed to allocate memory for reading \"%s\".\n", path);
        exit(EXIT_FAILURE);
    }

    size_t bytes_read = fread(buffer, sizeof(char), file_size, file);
    if (bytes_read < file_size) {
        fprintf(stderr, "Failed to read file \"%s\".\n", path);
        exit(EXIT_FAILURE);
    }
    buffer[bytes_read] = '\0';

    fclose(file);
    return buffer;
}

static void run_file(const char *path) {
    const char *dot = strrchr(path, '.');
    if (dot == NULL || strlen(dot) != 3 || strncmp(dot, ".ls", 3) != 0) {
        fprintf(stderr, "Error: Invalid file format.\n");
        fprintf(stderr, "Only files with '.ls' extension are supported.\n");
        fprintf(stderr, "Provided file: \"%s\"\n", path);
        exit(EXIT_FAILURE);
    }

    char *source = read_file(path);
    ls_vm_status status = ls_vm_interpret(&ls_vm, source);
    free(source);

    if (status != LS_VM_OK) {
        exit(status);
    }
}

int main(int argc, const char *argv[]) {
    ls_gc_init(&ls_gc);
    ls_vm_init(&ls_vm);

    if (argc == 1) {
        repl();
    } else if (argc == 2) {
        run_file(argv[1]);
    } else {
        fprintf(stderr, "Usage: %s [script]\n", argv[0]);
        exit(EXIT_FAILURE);
    }

    ls_vm_deinit(&ls_vm);
    ls_gc_deinit(&ls_gc);
    return 0;
}