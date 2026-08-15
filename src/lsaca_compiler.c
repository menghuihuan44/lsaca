#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "src/lsaca_common.h"
#include "src/lsaca_compiler.h"
#include "src/lsaca_scanner.h"

#if LS_DISASM_PRINT_CODE != 0
#include "src/lsaca_disasm.h"
#endif

typedef struct {
    ls_token_t current;
    ls_token_t previous;
    uint16_t reg_count;
    bool had_error;
    bool panic_mode;
} ls_parser_t;

typedef struct {
    uint8_t reg;
} ls_context_t;

/* Precedence levels in order from lowest to highest. */
typedef enum {
    LS_PREC_NONE,
    LS_PREC_ASSIGNMENT,  /* = */
    LS_PREC_OR,          /* or */
    LS_PREC_AND,         /* and */
    LS_PREC_EQUALITY,    /* == != */
    LS_PREC_COMPARISON,  /* < > <= >= */
    LS_PREC_TERM,        /* + - */
    LS_PREC_FACTOR,      /* * / */
    LS_PREC_UNARY,       /* ! - */
    LS_PREC_CALL,        /* . () */
    LS_PREC_PRIMARY
} ls_precedence;

typedef void (*ls_parse_fn)(ls_context_t *);

typedef struct {
    ls_parse_fn prefix;
    ls_parse_fn infix;
    ls_precedence precedence;
} parse_rule_t;

ls_parser_t ls_parser;
ls_chunk_t *compiling_chunk;

static ls_chunk_t *current_chunk(void) {
    return compiling_chunk;
}

static void error_at(ls_token_t *token, const char *message) {
    if (ls_parser.panic_mode) return;
    ls_parser.panic_mode = true;
    fprintf(stderr, "[line %zu] Error", token->line);

    if (token->type == LS_TOKEN_EOF) {
        fprintf(stderr,  " at end");
    } else if (token->type == LS_TOKEN_ERROR) {
        /* Nothing. */
    } else {
        fprintf(stderr, " at '%.*s'", (int)token->length, token->start);
    }

    fprintf(stderr, ": %s\n", message);
    ls_parser.had_error = true;
}

static void error(const char *message) {
    error_at(&ls_parser.previous, message);
}

static void error_at_current(const char *message) {
    error_at(&ls_parser.current, message);
}

static void advance(void) {
    ls_parser.previous = ls_parser.current;

    for (;;) {
        ls_parser.current = ls_scanner_scan_token();
        if (ls_parser.current.type != LS_TOKEN_ERROR) {
            break;
        }

        error_at_current(ls_parser.current.start);
    }
}

static void consume(ls_token_type type, const char *message) {
    if (ls_parser.current.type == type) {
        advance();
        return;
    }

    error_at_current(message);
}

static uint8_t alloc_reg(void) {
    if (ls_parser.reg_count >= LS_MAX_REGS) {
        error("Too many registers in use.");
        return 0; /* I don't know if this makes sense. */
    }
    return ls_parser.reg_count++;
}

static void free_reg(void) {
    if (ls_parser.reg_count > 0) {
        --ls_parser.reg_count;
    }
}

static void emit_inst(ls_inst_t inst) {
    ls_chunk_append(current_chunk(), inst, ls_parser.previous.line);
}

static void emit_return(uint8_t reg) {
    ls_inst_t inst = LS_ABC(LS_OP_RETURN, reg, 0, 0);
    emit_inst(inst);
}

static uint16_t make_constant(ls_any_t value) {
    size_t constant = ls_chunk_add_constant(current_chunk(), value);
    if (constant > UINT16_MAX) {
        error("Too many constants in one chunk.");
        return 0; /* I don't know if this makes sense. */
    }

    return (uint16_t)constant;
}

static void emit_constant(ls_any_t value, uint8_t dest_reg) {
    uint16_t const_idx = make_constant(value);
    ls_inst_t inst = LS_ABx(LS_OP_LOADK, dest_reg, const_idx);
    emit_inst(inst);
}

static void end_compiler(void) {
#if LS_DISASM_PRINT_CODE != 0
    if (!ls_parser.had_error) {
        ls_disasm_chunk(current_chunk(), "code");
    }
#endif
}

static void expression(ls_context_t *ctx);
static parse_rule_t *get_rule(ls_token_type type);
static void parse_precedence(ls_precedence precedence, ls_context_t *ctx);

static void binary(ls_context_t *ctx) {
    ls_token_type operator_type = ls_parser.previous.type;
    parse_rule_t *rule = get_rule(operator_type);
    uint8_t left_reg = ctx->reg;

    ls_context_t right_ctx;
    parse_precedence(rule->precedence + 1, &right_ctx);
    uint8_t right_reg = right_ctx.reg;

    uint8_t result_reg = alloc_reg();

    switch (operator_type) {
        case LS_TOKEN_PLUS: {
            ls_inst_t inst = LS_ABC(LS_OP_ADD, result_reg, left_reg, right_reg);
            emit_inst(inst);
            break;
        }
        case LS_TOKEN_MINUS: {
            ls_inst_t inst = LS_ABC(LS_OP_SUB, result_reg, left_reg, right_reg);
            emit_inst(inst);
            break;
        }
        case LS_TOKEN_STAR: {
            ls_inst_t inst = LS_ABC(LS_OP_MUL, result_reg, left_reg, right_reg);
            emit_inst(inst);
            break;
        }
        case LS_TOKEN_SLASH: {
            ls_inst_t inst = LS_ABC(LS_OP_DIV, result_reg, left_reg, right_reg);
            emit_inst(inst);
            break;
        }
        default: return;
    }

    ctx->reg = result_reg;
}

static void grouping(ls_context_t *ctx) {
    (void)ctx;
    expression(ctx);
    consume(LS_TOKEN_RIGHT_PAREN, "Expect ')' after expression.");
}

static void number(ls_context_t *ctx) {
    ctx->reg = alloc_reg();
    const char *dot = strchr(ls_parser.previous.start, '.');
    if (dot) {
        double value = strtod(ls_parser.previous.start, NULL);
        emit_constant(value, ctx->reg);
    } else {
        int64_t value = (int64_t)strtoll(ls_parser.previous.start, NULL, 0);
        emit_constant(value, ctx->reg);
    }
}

static void unary(ls_context_t *ctx) {
    ls_token_type operator_type = ls_parser.previous.type;

    parse_precedence(LS_PREC_UNARY, ctx);

    switch (operator_type) {
        case LS_TOKEN_MINUS: {
            ls_inst_t inst = LS_ABC(LS_OP_NEG, ctx->reg, 0, 0);
            emit_inst(inst);
            break;
        }
        default: return;
    }
}

parse_rule_t rules[] = {
    [LS_TOKEN_LEFT_PAREN]    = {grouping, NULL,   LS_PREC_NONE},
    [LS_TOKEN_RIGHT_PAREN]   = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_LEFT_BRACE]    = {NULL,     NULL,   LS_PREC_NONE}, 
    [LS_TOKEN_RIGHT_BRACE]   = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_COMMA]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_DOT]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_MINUS]         = {unary,    binary, LS_PREC_TERM},
    [LS_TOKEN_PLUS]          = {NULL,     binary, LS_PREC_TERM},
    [LS_TOKEN_COLON]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_SEMICOLON]     = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_SLASH]         = {NULL,     binary, LS_PREC_FACTOR},
    [LS_TOKEN_STAR]          = {NULL,     binary, LS_PREC_FACTOR},  

    [LS_TOKEN_BANG]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_BANG_EQUAL]    = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_EQUAL]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_EQUAL_EQUAL]   = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_GREATER]       = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_GREATER_EQUAL] = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_LESS]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_LESS_EQUAL]    = {NULL,     NULL,   LS_PREC_NONE},    

    [LS_TOKEN_IDENTIFIER]    = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_STRING]        = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_INT64]         = {number,   NULL,   LS_PREC_NONE},
    [LS_TOKEN_FLOAT64]       = {number,   NULL,   LS_PREC_NONE},    

    [LS_TOKEN_NULL]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_ANY]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_FALSE]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_TRUE]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_AND]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_OR]            = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_IF]            = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_ELSE]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_FOR]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_WHILE]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_RETURN]        = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_FUN]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_CLASS]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_THIS]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_SUPER]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_PRINT]         = {NULL,     NULL,   LS_PREC_NONE},    

    [LS_TOKEN_ERROR]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_EOF]           = {NULL,     NULL,   LS_PREC_NONE},
};

static void parse_precedence(ls_precedence precedence, ls_context_t *ctx) {
    advance();
    ls_parse_fn prefix_rule = get_rule(ls_parser.previous.type)->prefix;
    if (!prefix_rule) {
        error("Expect expression.");
        ctx->reg = 0;
        return;
    }

    prefix_rule(ctx);

    while (precedence <= get_rule(ls_parser.current.type)->precedence) {
        advance();
        ls_parse_fn infix_rule = get_rule(ls_parser.previous.type)->infix;
        infix_rule(ctx);
    }
}

static parse_rule_t *get_rule(ls_token_type type) {
    return &rules[type];
}

static void expression(ls_context_t *ctx) {
    parse_precedence(LS_PREC_ASSIGNMENT, ctx);
}

bool ls_compile(const char *source, ls_chunk_t *chunk) {
    ls_scanner_init(source);
    compiling_chunk = chunk;

    ls_parser.had_error = false;
    ls_parser.panic_mode = false;
    ls_parser.reg_count = 0;

    advance();

    ls_context_t ctx;
    expression(&ctx);
    consume(LS_TOKEN_EOF, "Expect end of expression.");

    emit_return(ctx.reg);
    end_compiler();
    return !ls_parser.had_error;
}