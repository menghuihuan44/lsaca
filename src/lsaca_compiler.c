#include <stdio.h>
#include "src/lsaca_common.h"
#include "src/lsaca_compiler.h"
#include "src/lsaca_scanner.h"

void ls_compile(const char *source) {
    ls_scanner_init(source);
    
    /* Temp */
    int line = -1;
    for (;;) {
        ls_token_t token = ls_scanner_scan_token();
        if (token.line != line) {
        printf("%4d ", token.line);
        line = token.line;
        } else {
        printf("   | ");
        }
        printf("%2d '%.*s'\n", token.type, (int)token.length, token.start); 

        if (token.type == LS_TOKEN_EOF) break;
    }
}