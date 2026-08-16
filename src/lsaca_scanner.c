#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include "src/lsaca_common.h"
#include "src/lsaca_scanner.h"

typedef struct {
    const char *start;
    const char *current;
    size_t line;
} ls_scanner_t;

ls_scanner_t ls_scanner;

void ls_scanner_init(const char *source) {
    ls_scanner.start = source;
    ls_scanner.current = source;
    ls_scanner.line = 1;
}

static bool is_at_end(void) {
    return *ls_scanner.current == '\0';
}

static bool is_next_at_end(void) {
    return ls_scanner.current[1] == '\0';
}

static char advance(void) {
    ++ls_scanner.current;
    return ls_scanner.current[-1];
}

static char peek(void) {
    return *ls_scanner.current;
}

static char peek_next(void) {
    if (is_at_end()) return '\0';
    return ls_scanner.current[1];
}

static bool match(char expected) {
    if (is_at_end() || *ls_scanner.current != expected) return false;
    ++ls_scanner.current;
    return true;
}

static ls_token_t make_token(ls_token_type type) {
    ls_token_t token;
    token.type = type;
    token.start = ls_scanner.start;
    token.length = (size_t)(ls_scanner.current - ls_scanner.start);
    token.line = ls_scanner.line;
    return token;
}

static ls_token_t error_token(const char *message) {
    ls_token_t token;
    token.type = LS_TOKEN_ERROR;
    token.start = message;
    token.length = strlen(message);
    token.line = ls_scanner.line;
    return token;
}

static void skip_whitespace(void) {
    for (;;) {
        char c = peek();
        switch (c) {
            case ' ':
            case '\r':
            case '\t':
                advance();
                break;
            case '\n':
                ++ls_scanner.line;
                advance();
                break;
            case '/':
                if (peek_next() == '/') {
                    while (!is_at_end() && peek() != '\n') {
                        advance();
                    }
                } else if (peek_next() == '*') {
                    while (!is_at_end() && !is_next_at_end() && (peek() != '*' || peek_next() != '/')) {
                        if (peek() == '\n') {
                            ++ls_scanner.line;
                        }
                        advance();
                    }
                    advance();
                    advance();
                } else {
                    return;
                }
                break;
            default:
                return;
        }
    }
}

static ls_token_type check_keyword(size_t start, size_t length, const char *rest, ls_token_type type) {
    if (ls_scanner.current - ls_scanner.start == start + length &&
    memcmp(ls_scanner.start + start, rest, length) == 0) {
        return type;
    }

    return LS_TOKEN_IDENTIFIER;
}

static ls_token_type identifier_type(void) {
    switch (ls_scanner.start[0]) {
        case 'a':
            if (ls_scanner.current - ls_scanner.start > 1) {
                switch (ls_scanner.start[1]) {
                    case 'n':
                        if (ls_scanner.current - ls_scanner.start > 2) {
                            switch (ls_scanner.start[2]) {
                                case 'd': return check_keyword(3, 0, "", LS_TOKEN_AND);
                                case 'y': return check_keyword(3, 0, "", LS_TOKEN_ANY);
                            }
                        }
                }
            }
            break;
        case 'c': return check_keyword(1, 4, "lass", LS_TOKEN_CLASS);
        case 'e': return check_keyword(1, 3, "lse", LS_TOKEN_ELSE);
        case 'f':
            if (ls_scanner.current - ls_scanner.start > 1) {
                switch (ls_scanner.start[1]) {
                    case 'a': return check_keyword(2, 3, "lse", LS_TOKEN_FALSE);
                    case 'o': return check_keyword(2, 1, "r", LS_TOKEN_FOR);
                    case 'u': return check_keyword(2, 1, "n", LS_TOKEN_FUN);
                }
            }
            break;
        case 'i': return check_keyword(1, 1, "f", LS_TOKEN_IF);
        case 'n': return check_keyword(1, 3, "ull", LS_TOKEN_NULL);
        case 'o': return check_keyword(1, 1, "r", LS_TOKEN_OR);
        case 'p': return check_keyword(1, 4, "rint", LS_TOKEN_PRINT);
        case 'r': return check_keyword(1, 5, "eturn", LS_TOKEN_RETURN);
        case 's': return check_keyword(1, 4, "uper", LS_TOKEN_SUPER);
        case 't':
            if (ls_scanner.current - ls_scanner.start > 1) {
                switch (ls_scanner.start[1]) {
                    case 'h': return check_keyword(2, 2, "is", LS_TOKEN_THIS);
                    case 'r': return check_keyword(2, 2, "ue", LS_TOKEN_TRUE);
                }
            }
            break;
        case 'w': return check_keyword(1, 4, "hile", LS_TOKEN_WHILE);
    }

    return LS_TOKEN_IDENTIFIER;
}

static ls_token_t identifier(void) {
    while (isalnum(peek()) || peek() == '_') {
        advance();
    }
    return make_token(identifier_type());
}

static ls_token_t number(void) {
    while (isdigit(peek())) {
        advance();
    }

    if (peek() == '.' && isdigit(peek_next())) {
        advance();
        while (isdigit(peek())) {
            advance();
        }
        return make_token(LS_TOKEN_FLOAT64);
    } else {
        return make_token(LS_TOKEN_INT64);
    }
}

static ls_token_t string(void) {
    while (!is_at_end() && peek() != '"') {
        if (peek() == '\n') {
            ++ls_scanner.line;
        }
        advance();
    }

    if (is_at_end()) return error_token("Unterminated string literal.");

    advance();
    return make_token(LS_TOKEN_STRING);
}

ls_token_t ls_scanner_scan_token(void) {
    skip_whitespace();
    ls_scanner.start = ls_scanner.current;

    if (is_at_end()) {
        return make_token(LS_TOKEN_EOF);
    }

    char c = advance();
    if (isalpha(c) || c == '_') return identifier();
    if (isdigit(c)) return number();

    switch(c) {
        case '(': return make_token(LS_TOKEN_LEFT_PAREN);
        case ')': return make_token(LS_TOKEN_RIGHT_PAREN);
        case '{': return make_token(LS_TOKEN_LEFT_BRACE);
        case '}': return make_token(LS_TOKEN_RIGHT_BRACE);
        case ':': return make_token(LS_TOKEN_COLON);
        case ';': return make_token(LS_TOKEN_SEMICOLON);
        case ',': return make_token(LS_TOKEN_COMMA);
        case '.': return make_token(LS_TOKEN_DOT);
        case '-': return make_token(LS_TOKEN_MINUS);
        case '+': return make_token(LS_TOKEN_PLUS);
        case '/': return make_token(LS_TOKEN_SLASH);
        case '*': return make_token(LS_TOKEN_STAR);
        case '!': return make_token(match('=') ? LS_TOKEN_BANG_EQUAL : LS_TOKEN_BANG);
        case '=': return make_token(match('=') ? LS_TOKEN_EQUAL_EQUAL : LS_TOKEN_EQUAL);
        case '<': return make_token(match('=') ? LS_TOKEN_LESS_EQUAL : LS_TOKEN_LESS);
        case '>': return make_token(match('=') ? LS_TOKEN_GREATER_EQUAL : LS_TOKEN_GREATER);
        case '"': return string();
    }

    return error_token("Unexpected character.");
}