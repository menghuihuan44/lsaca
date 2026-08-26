#ifndef LSACA_SCANNER_H
#define LSACA_SCANNER_H

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    LS_TOKEN_LEFT_PAREN, /* ( */
    LS_TOKEN_RIGHT_PAREN, /* ) */
    LS_TOKEN_LEFT_BRACE, /* { */
    LS_TOKEN_RIGHT_BRACE, /* } */
    LS_TOKEN_COMMA, /* , */
    LS_TOKEN_DOT, /* . */
    LS_TOKEN_MINUS, /* - */
    LS_TOKEN_PLUS, /* + */
    LS_TOKEN_COLON, /* : */
    LS_TOKEN_SEMICOLON, /* ; */
    LS_TOKEN_SLASH, /* / */
    LS_TOKEN_STAR, /* * */

    LS_TOKEN_BANG, /* ! */
    LS_TOKEN_BANG_EQUAL, /* != */
    LS_TOKEN_EQUAL, /* = */
    LS_TOKEN_EQUAL_EQUAL, /* == */
    LS_TOKEN_GREATER, /* > */
    LS_TOKEN_GREATER_EQUAL, /* >= */
    LS_TOKEN_LESS, /* < */
    LS_TOKEN_LESS_EQUAL, /* <= */

    LS_TOKEN_IDENTIFIER,
    LS_TOKEN_STRING,
    LS_TOKEN_INT64,
    LS_TOKEN_FLOAT64,

    LS_TOKEN_NULL,
    LS_TOKEN_ANY,
    LS_TOKEN_FALSE,
    LS_TOKEN_TRUE,
    LS_TOKEN_AND,
    LS_TOKEN_OR,
    LS_TOKEN_IF,
    LS_TOKEN_ELSE,
    LS_TOKEN_FOR,
    LS_TOKEN_WHILE,
    LS_TOKEN_RETURN,
    LS_TOKEN_FUN,
    LS_TOKEN_CLASS,
    LS_TOKEN_THIS,
    LS_TOKEN_SUPER,
    LS_TOKEN_PRINT, /* temp */

    LS_TOKEN_ERROR,
    LS_TOKEN_EOF
} ls_token_type;

typedef struct {
    ls_token_type type;
    const char *start;
    size_t length;
    size_t line;
} ls_token_t;

void ls_scanner_init(const char *source);
ls_token_t ls_scanner_scan_token(void);

#ifdef __cplusplus
}
#endif

#endif /* LSACA_SCANNER_H */