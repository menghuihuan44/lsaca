#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "src/lsaca_common.h"
#include "src/lsaca_compiler.h"
#include "src/lsaca_scanner.h"
#include "src/base/lsaca_gc.h"

#if LS_DISASM_PRINT_CODE != 0
#include "src/lsaca_disasm.h"
#endif

typedef struct {
    ls_token_t current;
    ls_token_t previous;
    bool had_error;
    bool panic_mode;
} ls_parser_t;

typedef struct {
    uint8_t reg;
    bool is_assign;
    bool is_temp;
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

typedef struct {
    ls_token_t name;
    size_t depth;
    uint8_t reg;
    bool ready;
} ls_scope_t;

typedef struct ls_loop_t {
    size_t start;
    size_t breaks[LS_UINT8_COUNT];
    size_t break_count;
    struct ls_loop_t *enclosing;
} ls_loop_t;

typedef struct {
    ls_scope_t scopes[LS_UINT8_COUNT];
    size_t scope_count;
    size_t scope_depth;
    uint16_t reg_top;
} ls_compiler_t;

ls_parser_t ls_parser;
ls_loop_t *current_loop = NULL;
ls_compiler_t *current = NULL;
ls_chunk_t *compiling_chunk;

static ls_chunk_t *current_chunk(void) {
    return compiling_chunk;
}

static void error_at(ls_token_t *token, const char *message) {
    if (ls_parser.panic_mode) return;
    ls_parser.panic_mode = true;
    fprintf(stderr, "[line %zu] Error", token->line);

    if (token->type == LS_TOKEN_EOF) {
        fprintf(stderr,  " at end of input");
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

static bool check(ls_token_type type) {
    return ls_parser.current.type == type;
}

static bool match(ls_token_type type) {
    if (!check(type)) return false;
    advance();
    return true;
}

static uint8_t alloc_reg(void) {
    if (current->reg_top >= LS_MAX_REGS) {
        error("Too many registers are in use.");
        return 0; /* I don't know if this makes sense. */
    }
    return current->reg_top++;
}

static void free_reg(void) {
    if (current->reg_top > 0) {
        --current->reg_top;
    }
}

static void free_regs_to(uint8_t target) {
    current->reg_top = target;
}

static void emit_inst(ls_inst_t inst) {
    ls_chunk_append(current_chunk(), inst, ls_parser.previous.line);
}

static size_t emit_jump(ls_opcode op, uint8_t reg) {
    op == LS_OP_JUMP_IF_FALSE ? emit_inst(LS_ABx(op, reg, 0)) : emit_inst(LS_Ax(op, 0));
    return current_chunk()->size - 1;
}

static void emit_loop(size_t loop_start) {
    int32_t offset = loop_start - current_chunk()->size - 1;
    if (-offset > (1 << LS_SIZE_Ax) - 1) {
        error("Loop body too large.");
    }
    emit_inst(LS_Ax(LS_OP_JUMP, offset));
}

static void emit_return(uint8_t reg1, uint8_t reg2) {
    ls_inst_t inst = LS_ABC(LS_OP_RETURN, reg1, reg2, 0);
    emit_inst(inst);
}

static uint16_t make_constant(ls_any_t value) {
    size_t constant = ls_chunk_add_constant(current_chunk(), value);
    if (constant > UINT16_MAX) {
        error("Too many constants in this chunk.");
        return 0; /* I don't know if this makes sense. */
    }

    return (uint16_t)constant;
}

static void emit_constant(ls_any_t value, uint8_t dest_reg) {
    uint16_t const_idx = make_constant(value);
    ls_inst_t inst = LS_ABx(LS_OP_LOADK, dest_reg, const_idx);
    emit_inst(inst);
}

static void patch_jump(size_t offset) {
    int32_t jump = current_chunk()->size - offset - 1;

    if (jump > (1 << LS_SIZE_Ax) - 1) {
        error("Too much code to jump over.");
    }

    ls_inst_t *inst = &current_chunk()->codes[offset];
    *inst = LS_OP(*inst) == LS_OP_JUMP_IF_FALSE ? LS_ABx(LS_OP_JUMP_IF_FALSE, LS_RA(*inst), jump) : LS_Ax(LS_OP_JUMP, jump);
}

static void compiler_init(ls_compiler_t *compiler) {
    compiler->scope_count = 0;
    compiler->scope_depth = 0;
    compiler->reg_top = 0;
    current = compiler;
}

static void end_compiler(void) {
    emit_return(1, 0); /* temp */

#if LS_DISASM_PRINT_CODE != 0
    if (!ls_parser.had_error) {
        ls_disasm_chunk(current_chunk(), "code");
    }
#endif
}

static void begin_scope(void) {
    ++current->scope_depth;
}

static void end_scope(void) {
    --current->scope_depth;

    uint8_t restore_to = current->reg_top;
    while (current->scope_count > 0 && current->scopes[current->scope_count - 1].depth > current->scope_depth) {
        restore_to = current->scopes[current->scope_count - 1].reg;
        --current->scope_count;
    }
    current->reg_top = restore_to;
}

static void expression(ls_context_t *ctx);
static void statement(ls_context_t *ctx);
static void declaration(ls_context_t *ctx);
static parse_rule_t *get_rule(ls_token_type type);
static void parse_precedence(ls_precedence precedence, ls_context_t *ctx);

static void binary(ls_context_t *ctx) {
    ls_token_type operator_type = ls_parser.previous.type;
    parse_rule_t *rule = get_rule(operator_type);

    uint8_t left_reg = ctx->reg;
    bool left_is_temp = ctx->is_temp;

    ls_context_t right_ctx = {0};
    parse_precedence(rule->precedence + 1, &right_ctx);
    uint8_t right_reg = right_ctx.reg;
    bool right_is_temp = right_ctx.is_temp;

    uint8_t result_reg;
    if (left_is_temp) {
        result_reg = left_reg;
    } else if (right_is_temp) {
        result_reg = right_reg;
    } else {
        result_reg = alloc_reg();
    }

    switch (operator_type) {
        case LS_TOKEN_BANG_EQUAL: {
            emit_inst(LS_ABC(LS_OP_BANG_EQUAL, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_EQUAL_EQUAL: {
            emit_inst(LS_ABC(LS_OP_EQUAL_EQUAL, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_GREATER: {
            emit_inst(LS_ABC(LS_OP_GREATER, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_GREATER_EQUAL: {
            emit_inst(LS_ABC(LS_OP_GREATER_EQUAL, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_LESS: {
            emit_inst(LS_ABC(LS_OP_LESS, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_LESS_EQUAL: {
            emit_inst(LS_ABC(LS_OP_LESS_EQUAL, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_PLUS: {
            emit_inst(LS_ABC(LS_OP_ADD, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_MINUS: {
            emit_inst(LS_ABC(LS_OP_SUB, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_STAR: {
            emit_inst(LS_ABC(LS_OP_MUL, result_reg, left_reg, right_reg));
            break;
        }
        case LS_TOKEN_SLASH: {
            emit_inst(LS_ABC(LS_OP_DIV, result_reg, left_reg, right_reg));
            break;
        }
        default: {
            error("Unknown binary operator.");
            return;
        }
    }

    free_regs_to(result_reg + 1);

    ctx->reg = result_reg;
    ctx->is_temp = true;
}

static void literal(ls_context_t *ctx) {
    ctx->reg = alloc_reg();
    ctx->is_temp = true;
    switch (ls_parser.previous.type) {
        case LS_TOKEN_NULL: {
            emit_inst(LS_ABC(LS_OP_LOADNULL, ctx->reg, 0, 0));
            break;
        }
        case LS_TOKEN_FALSE: {
            emit_inst(LS_ABC(LS_OP_LOADFALSE, ctx->reg, 0, 0));
            break;
        }
        case LS_TOKEN_TRUE: {
            emit_inst(LS_ABC(LS_OP_LOADTRUE, ctx->reg, 0, 0));
            break;
        }
        default: {
            error("Unknown literal operator.");
            return;
        }
    }
}

static void grouping(ls_context_t *ctx) {
    expression(ctx);
    consume(LS_TOKEN_RIGHT_PAREN, "Expected ')' after the expression.");
}

static void number(ls_context_t *ctx) {
    ctx->reg = alloc_reg();
    ctx->is_temp = true;
    switch (ls_parser.previous.type) {
        case LS_TOKEN_INT64: {
            int64_t value = (int64_t)strtoll(ls_parser.previous.start, NULL, 0);
            emit_constant(LS_I64_ANY(value), ctx->reg);
            break;
        }
        case LS_TOKEN_FLOAT64: {
            double value = strtod(ls_parser.previous.start, NULL);
            emit_constant(LS_F64_ANY(value), ctx->reg);
            break;
        }
        default: {
            error("Unknown number operator.");
            return;
        }
    }
}

static void string(ls_context_t *ctx) {
    ctx->reg = alloc_reg();
    ctx->is_temp = true;
    ls_string_t *str = ls_string_create_with_len(ls_parser.previous.start + 1, ls_parser.previous.length - 2);
    emit_constant(LS_STR_ANY(str), ctx->reg);
}

static bool identifiers_equal(ls_token_t *a, ls_token_t *b) {
    if (a->length != b->length) return false;
    return memcmp(a->start, b->start, a->length) == 0;
}

static size_t resolve_scope(ls_compiler_t *compiler, ls_token_t *name) {
    for (int64_t i = (int64_t)compiler->scope_count - 1; i >= 0; --i) {
        ls_scope_t *scope = &compiler->scopes[i];
        if (identifiers_equal(name, &scope->name)) {
            if (!scope->ready) {
                error_at(name, "Cannot read local variable in its own initializer.");
                return (size_t)-1;
            }
            return i;
        }
    }

    error_at(name, "Undefined variable.");
    return (size_t)-1;
}

static void variable(ls_context_t *ctx) {
    size_t idx = resolve_scope(current, &ls_parser.previous);
    if (idx == (size_t)-1) return;
    uint8_t var_reg = current->scopes[idx].reg;

    if (ctx->is_assign && match(LS_TOKEN_EQUAL)) {
        uint8_t saved_top = current->reg_top;
        ls_context_t rhs_ctx = {0};
        expression(&rhs_ctx);

        if (rhs_ctx.reg != var_reg) {
            emit_inst(LS_ABC(LS_OP_MOVE, var_reg, rhs_ctx.reg, 0));
        }
        free_regs_to(saved_top);

        ctx->reg = var_reg;
        ctx->is_temp = false;
        ctx->is_assign = false;
    } else {
        ctx->reg = var_reg;
        ctx->is_temp = false;
    }
}

static void unary(ls_context_t *ctx) {
    ls_token_type operator_type = ls_parser.previous.type;

    parse_precedence(LS_PREC_UNARY, ctx);

    uint8_t result_reg;
    if (ctx->is_temp) {
        result_reg = ctx->reg;
    } else {
        result_reg = alloc_reg();
    }
    
    switch (operator_type) {
        case LS_TOKEN_MINUS: {
            emit_inst(LS_ABC(LS_OP_NEG, result_reg, ctx->reg, 0));
            break;
        }
        case LS_TOKEN_BANG: {
            emit_inst(LS_ABC(LS_OP_NOT, result_reg, ctx->reg, 0));
            break;
        }
        default: {
            error("Unknown unary operator.");
            return;
        }
    }

    free_regs_to(result_reg + 1);
    ctx->reg = result_reg;
    ctx->is_temp = true;
}

static void and_(ls_context_t *ctx) {
    uint8_t left_reg = ctx->reg;
    bool left_is_temp = ctx->is_temp;

    uint8_t result_reg = left_is_temp ? left_reg : alloc_reg();
    if (!left_is_temp) {
        emit_inst(LS_ABC(LS_OP_MOVE, result_reg, left_reg, 0));
    }

    size_t end_jump = emit_jump(LS_OP_JUMP_IF_FALSE, result_reg);

    uint8_t saved_top = current->reg_top;
    ls_context_t right_ctx = {0};
    parse_precedence(LS_PREC_AND, &right_ctx);

    if (right_ctx.reg != result_reg) {
        emit_inst(LS_ABC(LS_OP_MOVE, result_reg, right_ctx.reg, 0));
    }
    free_regs_to(saved_top);

    patch_jump(end_jump);

    ctx->reg = result_reg;
    ctx->is_temp = true;
}

static void or_(ls_context_t *ctx) {
    uint8_t left_reg = ctx->reg;
    bool left_is_temp = ctx->is_temp;

    uint8_t result_reg = left_is_temp ? left_reg : alloc_reg();
    if (!left_is_temp) {
        emit_inst(LS_ABC(LS_OP_MOVE, result_reg, left_reg, 0));
    }

    size_t else_jump = emit_jump(LS_OP_JUMP_IF_FALSE, result_reg);
    size_t end_jump = emit_jump(LS_OP_JUMP, 0);

    patch_jump(else_jump);

    uint8_t saved_top = current->reg_top;
    ls_context_t right_ctx = {0};
    parse_precedence(LS_PREC_OR, &right_ctx);

    if (right_ctx.reg != result_reg) {
        emit_inst(LS_ABC(LS_OP_MOVE, result_reg, right_ctx.reg, 0));
    }
    free_regs_to(saved_top);

    patch_jump(end_jump);

    ctx->reg = result_reg;
    ctx->is_temp = true;
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

    [LS_TOKEN_BANG]          = {unary,    NULL,   LS_PREC_NONE},
    [LS_TOKEN_BANG_EQUAL]    = {NULL,     binary, LS_PREC_EQUALITY},
    [LS_TOKEN_EQUAL]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_EQUAL_EQUAL]   = {NULL,     binary, LS_PREC_EQUALITY},
    [LS_TOKEN_GREATER]       = {NULL,     binary, LS_PREC_COMPARISON},
    [LS_TOKEN_GREATER_EQUAL] = {NULL,     binary, LS_PREC_COMPARISON},
    [LS_TOKEN_LESS]          = {NULL,     binary, LS_PREC_COMPARISON},
    [LS_TOKEN_LESS_EQUAL]    = {NULL,     binary, LS_PREC_COMPARISON},

    [LS_TOKEN_IDENTIFIER]    = {variable, NULL,   LS_PREC_NONE},
    [LS_TOKEN_STRING]        = {string,   NULL,   LS_PREC_NONE},
    [LS_TOKEN_INT64]         = {number,   NULL,   LS_PREC_NONE},
    [LS_TOKEN_FLOAT64]       = {number,   NULL,   LS_PREC_NONE},

    [LS_TOKEN_NULL]          = {literal,  NULL,   LS_PREC_NONE},
    [LS_TOKEN_ANY]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_FALSE]         = {literal,  NULL,   LS_PREC_NONE},
    [LS_TOKEN_TRUE]          = {literal,  NULL,   LS_PREC_NONE},
    [LS_TOKEN_AND]           = {NULL,     and_,   LS_PREC_AND},
    [LS_TOKEN_OR]            = {NULL,     or_,    LS_PREC_OR},
    [LS_TOKEN_IF]            = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_ELSE]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_FOR]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_WHILE]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_RETURN]        = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_FUN]           = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_CLASS]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_THIS]          = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_SUPER]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_PRINT]         = {NULL,     NULL,   LS_PREC_NONE}, /* temp */

    [LS_TOKEN_ERROR]         = {NULL,     NULL,   LS_PREC_NONE},
    [LS_TOKEN_EOF]           = {NULL,     NULL,   LS_PREC_NONE},
};

static void parse_precedence(ls_precedence precedence, ls_context_t *ctx) {
    advance();
    ls_parse_fn prefix_rule = get_rule(ls_parser.previous.type)->prefix;
    if (!prefix_rule) {
        error("Expected expression.");
        ctx->reg = 0;
        return;
    }

    ctx->is_assign = precedence <= LS_PREC_ASSIGNMENT;
    prefix_rule(ctx);

    while (precedence <= get_rule(ls_parser.current.type)->precedence) {
        advance();
        ls_parse_fn infix_rule = get_rule(ls_parser.previous.type)->infix;
        infix_rule(ctx);
    }

    if (ctx->is_assign && match(LS_TOKEN_EQUAL)) {
        error("Invalid assignment target."); 
    }
}

static void add_scope(ls_token_t name) {
    if (current->scope_count == LS_UINT8_COUNT) {
        error("Too many variables in scope.");
        return;
    }

    ls_scope_t *scope = &current->scopes[current->scope_count++];
    scope->name = name;
    scope->depth = current->scope_depth;
    scope->reg = alloc_reg();
    scope->ready = false;
}

static void declare_variable(void) {
    ls_token_t *name = &ls_parser.previous;
    for (int64_t i = (int64_t)current->scope_count - 1; i >= 0; --i) {
        ls_scope_t *scope = &current->scopes[i];
        if (scope->depth < current->scope_depth) {
            break;
        }

        if (identifiers_equal(name, &scope->name)) {
            error("Already a variable with this name in this scope.");
        }
    }

    add_scope(*name);
}

static parse_rule_t *get_rule(ls_token_type type) {
    return &rules[type];
}

static void expression(ls_context_t *ctx) {
    parse_precedence(LS_PREC_ASSIGNMENT, ctx);
}

static void var_declaration(ls_context_t *ctx) {
    consume(LS_TOKEN_IDENTIFIER, "Expected variable name.");
    declare_variable();
    uint8_t reg = current->scopes[current->scope_count - 1].reg;

    uint8_t saved_top = current->reg_top;
    if (match(LS_TOKEN_EQUAL)) {
        ls_context_t init_ctx = {0};
        expression(&init_ctx);
        if (init_ctx.reg != reg) {
            emit_inst(LS_ABC(LS_OP_MOVE, reg, init_ctx.reg, 0));
        }
        free_regs_to(saved_top);
    } else {
        emit_inst(LS_ABC(LS_OP_LOADNULL, reg, 0, 0));
    }
    current->scopes[current->scope_count - 1].ready = true;

    consume(LS_TOKEN_SEMICOLON, "Expected ';' after declaration.");
}

static void expression_statement(ls_context_t *ctx) {
    uint8_t saved_top = current->reg_top;
    expression(ctx);
    consume(LS_TOKEN_SEMICOLON, "Expect ';' after expression.");
    free_regs_to(saved_top);
}

static void block(ls_context_t *ctx) {
    while (!check(LS_TOKEN_RIGHT_BRACE) && !check(LS_TOKEN_EOF)) {
        declaration(ctx);
    }

    consume(LS_TOKEN_RIGHT_BRACE, "Expect '}' after block.");
}

static void if_statement(ls_context_t *ctx) {
    consume(LS_TOKEN_LEFT_PAREN, "Expect '(' after 'if'.");
    uint8_t saved_top = current->reg_top;
    ls_context_t cond_ctx = {0};
    expression(&cond_ctx);
    consume(LS_TOKEN_RIGHT_PAREN, "Expect ')' after condition.");

    size_t then_jump = emit_jump(LS_OP_JUMP_IF_FALSE, cond_ctx.reg);
    free_regs_to(saved_top);
    statement(ctx);

    if (match(LS_TOKEN_ELSE)) {
        size_t else_jump = emit_jump(LS_OP_JUMP, 0);
        patch_jump(then_jump);
        statement(ctx);
        patch_jump(else_jump);
    } else {
        patch_jump(then_jump);
    }
}

static void continue_statement(ls_context_t *ctx) {
    if (current_loop == NULL) {
        error("'continue' outside of a loop.");
        return;
    }
    consume(LS_TOKEN_SEMICOLON, "Expect ';' after 'continue'.");

    emit_loop(current_loop->start);
}

static void break_statement(ls_context_t *ctx) {
    if (current_loop == NULL) {
        error("'break' outside of a loop.");
        return;
    }
    consume(LS_TOKEN_SEMICOLON, "Expect ';' after 'break'.");
    
    current_loop->breaks[current_loop->break_count++] = emit_jump(LS_OP_JUMP, 0);
}

static void while_statement(ls_context_t *ctx) {
    ls_loop_t loop;
    loop.enclosing = current_loop;
    loop.break_count = 0;
    current_loop = &loop;

    consume(LS_TOKEN_LEFT_PAREN, "Expected '(' after 'while'.");
    loop.start = current_chunk()->size;
    uint8_t saved_top = current->reg_top;
    ls_context_t cond_ctx = {0};
    expression(&cond_ctx);
    consume(LS_TOKEN_RIGHT_PAREN, "Expected ')' after condition.");

    size_t exit_jump = emit_jump(LS_OP_JUMP_IF_FALSE, cond_ctx.reg);
    free_regs_to(saved_top);
    statement(ctx);

    emit_loop(loop.start);
    patch_jump(exit_jump);

    for (size_t i = 0; i < loop.break_count; ++i) {
        patch_jump(loop.breaks[i]);
    }
    current_loop = loop.enclosing;
}

static void for_statement(ls_context_t *ctx) {
    ls_loop_t loop;
    loop.enclosing = current_loop;
    loop.break_count = 0;
    current_loop = &loop;

    begin_scope();
    consume(LS_TOKEN_LEFT_PAREN, "Expect '(' after 'for'.");
    if (match(LS_TOKEN_SEMICOLON)) {
        /* No initializer. */
    } else if (match(LS_TOKEN_ANY)) {
        var_declaration(ctx);
    } else {
        expression_statement(ctx);
    }

    size_t loop_start = current_chunk()->size;
    loop.start = loop_start;
    size_t exit_jump = (size_t)-1;
    if (!match(LS_TOKEN_SEMICOLON)) {
        uint8_t saved_top = current->reg_top;
        ls_context_t cond_ctx = {0};
        expression(&cond_ctx);
        consume(LS_TOKEN_SEMICOLON, "Expect ';' after loop condition.");

        exit_jump = emit_jump(LS_OP_JUMP_IF_FALSE, cond_ctx.reg);
        free_regs_to(saved_top);
    }

    if (!match(LS_TOKEN_RIGHT_PAREN)) {
        size_t body_jump = emit_jump(LS_OP_JUMP, 0);
        size_t increment_start = current_chunk()->size;
        loop.start = increment_start;
        uint8_t saved_top = current->reg_top;
        ls_context_t cond_ctx = {0};
        expression(&cond_ctx);
        consume(LS_TOKEN_RIGHT_PAREN, "Expect ')' after for clauses.");
        free_regs_to(saved_top);

        emit_loop(loop_start);
        loop_start = increment_start;
        patch_jump(body_jump);
    }

    statement(ctx);
    emit_loop(loop_start);

    if (exit_jump != (size_t)-1) {
        patch_jump(exit_jump);
    }

    for (size_t i = 0; i < loop.break_count; ++i) {
        patch_jump(loop.breaks[i]);
    }
    current_loop = loop.enclosing;

    end_scope();
}

static void print_statement(ls_context_t *ctx) {
    uint8_t saved_top = current->reg_top;
    expression(ctx);
    consume(LS_TOKEN_SEMICOLON, "Expect ';' after value.");
    emit_inst(LS_ABC(LS_OP_PRINT, ctx->reg, 0 , 0));
    free_regs_to(saved_top);
}

static void synchronize(void) {
    ls_parser.panic_mode = false;

    while (ls_parser.current.type != LS_TOKEN_EOF) {
        if (ls_parser.previous.type == LS_TOKEN_SEMICOLON) return;
        switch (ls_parser.current.type) {
            case LS_TOKEN_ANY:
            case LS_TOKEN_IF:
            case LS_TOKEN_FOR:
            case LS_TOKEN_WHILE:
            case LS_TOKEN_RETURN:
            case LS_TOKEN_FUN:
            case LS_TOKEN_CLASS:
            case LS_TOKEN_PRINT:
                return;
            default: ;
        }

        advance();
    }
}

static void declaration(ls_context_t *ctx) {
    if (match(LS_TOKEN_ANY)) {
        var_declaration(ctx);
    } else {
        statement(ctx);
    }

    if (ls_parser.panic_mode) synchronize();
}

static void statement(ls_context_t *ctx) {
    if (match(LS_TOKEN_PRINT)) {
        print_statement(ctx); /* temp */
    } else if (match(LS_TOKEN_LEFT_BRACE)) {
        begin_scope();
        block(ctx);
        end_scope();
    } else if (match(LS_TOKEN_IF)) {
        if_statement(ctx);
    } else if (match(LS_TOKEN_CONTINUE)) {
        continue_statement(ctx);
    } else if (match(LS_TOKEN_BREAK)) {
        break_statement(ctx);
    } else if (match(LS_TOKEN_WHILE)) {
        while_statement(ctx);
    } else if (match((LS_TOKEN_FOR))) {
        for_statement(ctx);
    } else {
        expression_statement(ctx);
    }
}

bool ls_compile(const char *source, ls_chunk_t *chunk) {
    ls_scanner_init(source);
    ls_compiler_t compiler;
    compiler_init(&compiler);
    compiling_chunk = chunk;

    ls_parser.had_error = false;
    ls_parser.panic_mode = false;

    advance();

    ls_context_t ctx = {0};
    while (!match(LS_TOKEN_EOF)) {
        declaration(&ctx);
    }

    end_compiler();
    return !ls_parser.had_error;
}