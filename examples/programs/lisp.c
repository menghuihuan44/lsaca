// lisp.c - C17 counterpart of examples/programs/lisp.ls.
//
// The two programs implement the same tiny Lisp interpreter and must produce
// byte-identical output: they evaluate one embedded script, print the values
// produced by the (print ...) special form, report bookkeeping counts, show
// two parse-error demos, and optionally evaluate a script from a file.

#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

// ---------------------------------------------------------------- utilities

static void *xmalloc(size_t size) {
    void *block = malloc(size == 0 ? 1 : size);
    if (block == NULL) {
        fprintf(stderr, "lisp: out of memory\n");
        exit(1);
    }
    return block;
}

static void *xrealloc(void *old_block, size_t size) {
    void *block = realloc(old_block, size == 0 ? 1 : size);
    if (block == NULL) {
        fprintf(stderr, "lisp: out of memory\n");
        exit(1);
    }
    return block;
}

static char *xstrdup(const char *text) {
    size_t length = strlen(text);
    char *copy = xmalloc(length + 1);
    memcpy(copy, text, length + 1);
    return copy;
}

static char *format_string(const char *format, ...) {
    va_list args;
    va_start(args, format);
    va_list copy;
    va_copy(copy, args);
    int needed = vsnprintf(NULL, 0, format, copy);
    va_end(copy);
    if (needed < 0) {
        va_end(args);
        return xstrdup("format error");
    }
    char *text = xmalloc((size_t)needed + 1);
    vsnprintf(text, (size_t)needed + 1, format, args);
    va_end(args);
    return text;
}

typedef struct {
    char *data;
    size_t length;
    size_t capacity;
} Buf;

static void buf_init(Buf *buffer) {
    buffer->capacity = 64;
    buffer->length = 0;
    buffer->data = xmalloc(buffer->capacity);
    buffer->data[0] = '\0';
}

static void buf_append_n(Buf *buffer, const char *text, size_t length) {
    if (buffer->length + length + 1 > buffer->capacity) {
        while (buffer->length + length + 1 > buffer->capacity) {
            buffer->capacity *= 2;
        }
        buffer->data = xrealloc(buffer->data, buffer->capacity);
    }
    memcpy(buffer->data + buffer->length, text, length);
    buffer->length += length;
    buffer->data[buffer->length] = '\0';
}

static void buf_append(Buf *buffer, const char *text) {
    buf_append_n(buffer, text, strlen(text));
}

static void buf_append_char(Buf *buffer, char ch) {
    buf_append_n(buffer, &ch, 1);
}

// ------------------------------------------------------------------- values

typedef struct Sexp Sexp;
typedef struct {
    Sexp **items;
    size_t length;
    size_t capacity;
} SexpVec;

typedef struct {
    char **params;
    size_t param_count;
    SexpVec body;
    SexpVec captured;
} LambdaData;

typedef enum {
    SEXP_NUM,
    SEXP_BOOL,
    SEXP_SYM,
    SEXP_STR,
    SEXP_LIST,
    SEXP_LAMBDA
} SexpKind;

struct Sexp {
    SexpKind kind;
    long long number;
    int boolean;
    char *text;
    SexpVec list;
    LambdaData lambda;
};

static Sexp *make_num(long long value) {
    Sexp *expr = xmalloc(sizeof(Sexp));
    memset(expr, 0, sizeof(Sexp));
    expr->kind = SEXP_NUM;
    expr->number = value;
    return expr;
}

static Sexp *make_bool(int value) {
    Sexp *expr = xmalloc(sizeof(Sexp));
    memset(expr, 0, sizeof(Sexp));
    expr->kind = SEXP_BOOL;
    expr->boolean = value;
    return expr;
}

static Sexp *make_sym(const char *name) {
    Sexp *expr = xmalloc(sizeof(Sexp));
    memset(expr, 0, sizeof(Sexp));
    expr->kind = SEXP_SYM;
    expr->text = xstrdup(name);
    return expr;
}

static Sexp *make_str(const char *text) {
    Sexp *expr = xmalloc(sizeof(Sexp));
    memset(expr, 0, sizeof(Sexp));
    expr->kind = SEXP_STR;
    expr->text = xstrdup(text);
    return expr;
}

static Sexp *make_list(SexpVec items);

static Sexp *make_lambda(char **params, size_t param_count, SexpVec body, SexpVec captured) {
    Sexp *expr = xmalloc(sizeof(Sexp));
    memset(expr, 0, sizeof(Sexp));
    expr->kind = SEXP_LAMBDA;
    expr->lambda.params = params;
    expr->lambda.param_count = param_count;
    expr->lambda.body = body;
    expr->lambda.captured = captured;
    return expr;
}

static void vec_init(SexpVec *vec) {
    vec->items = NULL;
    vec->length = 0;
    vec->capacity = 0;
}

static void vec_push(SexpVec *vec, Sexp *item) {
    if (vec->length == vec->capacity) {
        size_t next = vec->capacity == 0 ? 4 : vec->capacity * 2;
        vec->items = xrealloc(vec->items, next * sizeof(Sexp *));
        vec->capacity = next;
    }
    vec->items[vec->length] = item;
    vec->length += 1;
}

static Sexp *sexp_clone(const Sexp *expr);

static SexpVec vec_clone(const SexpVec *vec) {
    SexpVec copy;
    vec_init(&copy);
    for (size_t index = 0; index < vec->length; index += 1) {
        vec_push(&copy, sexp_clone(vec->items[index]));
    }
    return copy;
}

static Sexp *make_list(SexpVec items) {
    Sexp *expr = xmalloc(sizeof(Sexp));
    memset(expr, 0, sizeof(Sexp));
    expr->kind = SEXP_LIST;
    expr->list = items;
    return expr;
}

static Sexp *sexp_clone(const Sexp *expr) {
    switch (expr->kind) {
    case SEXP_NUM:
        return make_num(expr->number);
    case SEXP_BOOL:
        return make_bool(expr->boolean);
    case SEXP_SYM:
        return make_sym(expr->text);
    case SEXP_STR:
        return make_str(expr->text);
    case SEXP_LIST:
        return make_list(vec_clone(&expr->list));
    case SEXP_LAMBDA: {
        char **params = xmalloc((expr->lambda.param_count == 0 ? 1 : expr->lambda.param_count) * sizeof(char *));
        for (size_t index = 0; index < expr->lambda.param_count; index += 1) {
            params[index] = xstrdup(expr->lambda.params[index]);
        }
        return make_lambda(params, expr->lambda.param_count, vec_clone(&expr->lambda.body), vec_clone(&expr->lambda.captured));
    }
    }
    return make_bool(0);
}

// ------------------------------------------------------------ output helpers

static void quote_text(Buf *out, const char *text) {
    buf_append_char(out, '"');
    const char *cursor = text;
    while (*cursor != '\0') {
        char ch = *cursor;
        if (ch == '"') {
            buf_append(out, "\\\"");
        } else if (ch == '\\') {
            buf_append(out, "\\\\");
        } else if (ch == '\n') {
            buf_append(out, "\\n");
        } else if (ch == '\t') {
            buf_append(out, "\\t");
        } else {
            buf_append_char(out, ch);
        }
        cursor += 1;
    }
    buf_append_char(out, '"');
}

static void display_into(Buf *out, const Sexp *expr);

static char *display(const Sexp *expr) {
    Buf out;
    buf_init(&out);
    display_into(&out, expr);
    return out.data;
}

static void display_into(Buf *out, const Sexp *expr) {
    switch (expr->kind) {
    case SEXP_NUM:
        buf_append(out, format_string("%lld", expr->number));
        return;
    case SEXP_BOOL:
        buf_append(out, expr->boolean ? "true" : "false");
        return;
    case SEXP_SYM:
        buf_append(out, expr->text);
        return;
    case SEXP_STR:
        quote_text(out, expr->text);
        return;
    case SEXP_LAMBDA:
        buf_append(out, format_string("<lambda/%zu>", expr->lambda.param_count));
        return;
    case SEXP_LIST:
        buf_append_char(out, '(');
        for (size_t index = 0; index < expr->list.length; index += 1) {
            if (index > 0) {
                buf_append_char(out, ' ');
            }
            display_into(out, expr->list.items[index]);
        }
        buf_append_char(out, ')');
        return;
    }
}

static int is_truthy(const Sexp *expr) {
    if (expr->kind == SEXP_BOOL) {
        return expr->boolean;
    }
    if (expr->kind == SEXP_LIST) {
        return expr->list.length > 0;
    }
    return 1;
}

// --------------------------------------------------------------- environment

static Sexp *binding_pair(const char *name, Sexp *value) {
    SexpVec pair;
    vec_init(&pair);
    vec_push(&pair, make_sym(name));
    vec_push(&pair, value);
    return make_list(pair);
}

static void env_push(SexpVec *env, const char *name, Sexp *value) {
    vec_push(env, binding_pair(name, value));
}

static int binding_name(const Sexp *entry, char **out_name) {
    if (entry->kind == SEXP_LIST && entry->list.length == 2) {
        Sexp *first = entry->list.items[0];
        if (first->kind == SEXP_SYM) {
            *out_name = first->text;
            return 1;
        }
    }
    return 0;
}

static Sexp *env_lookup(SexpVec *env, const char *name) {
    size_t index = env->length;
    while (index > 0) {
        index -= 1;
        Sexp *entry = env->items[index];
        if (entry->kind == SEXP_LIST && entry->list.length == 2) {
            Sexp *key = entry->list.items[0];
            if (key->kind == SEXP_SYM && strcmp(key->text, name) == 0) {
                return sexp_clone(entry->list.items[1]);
            }
        }
    }
    return NULL;
}

static int env_set(SexpVec *env, const char *name, Sexp *value) {
    size_t index = env->length;
    while (index > 0) {
        index -= 1;
        Sexp *entry = env->items[index];
        int matched = 0;
        if (entry->kind == SEXP_LIST && entry->list.length == 2) {
            Sexp *key = entry->list.items[0];
            if (key->kind == SEXP_SYM && strcmp(key->text, name) == 0) {
                matched = 1;
            }
        }
        if (matched) {
            env->items[index] = binding_pair(name, value);
            return 1;
        }
    }
    return 0;
}

// ------------------------------------------------------------------- parser

typedef struct {
    uint32_t *chars;
    size_t count;
    size_t index;
    size_t line;
    size_t column;
} Parser;

static uint32_t *utf8_decode(const char *text, size_t *out_count) {
    size_t length = strlen(text);
    uint32_t *chars = xmalloc((length == 0 ? 1 : length) * sizeof(uint32_t));
    size_t count = 0;
    size_t index = 0;
    while (index < length) {
        unsigned char first = (unsigned char)text[index];
        uint32_t value = 0;
        size_t width = 1;
        if (first < 0x80) {
            value = first;
            width = 1;
        } else if ((first & 0xE0) == 0xC0) {
            value = first & 0x1F;
            width = 2;
        } else if ((first & 0xF0) == 0xE0) {
            value = first & 0x0F;
            width = 3;
        } else if ((first & 0xF8) == 0xF0) {
            value = first & 0x07;
            width = 4;
        } else {
            value = 0xFFFD;
            width = 1;
        }
        if (width > 1) {
            for (size_t offset = 1; offset < width; offset += 1) {
                if (index + offset >= length) {
                    value = 0xFFFD;
                    width = offset;
                    break;
                }
                unsigned char next = (unsigned char)text[index + offset];
                value = (value << 6) | (next & 0x3F);
            }
        }
        chars[count] = value;
        count += 1;
        index += width;
    }
    *out_count = count;
    return chars;
}

static void parser_init(Parser *parser, const char *text) {
    parser->chars = utf8_decode(text, &parser->count);
    parser->index = 0;
    parser->line = 1;
    parser->column = 1;
}

static uint32_t parser_peek(const Parser *parser) {
    if (parser->index >= parser->count) {
        return 0;
    }
    return parser->chars[parser->index];
}

static uint32_t parser_peek_next(const Parser *parser) {
    if (parser->index + 1 >= parser->count) {
        return 0;
    }
    return parser->chars[parser->index + 1];
}

static uint32_t parser_advance(Parser *parser) {
    uint32_t ch = parser_peek(parser);
    if (ch == 0) {
        return ch;
    }
    parser->index += 1;
    if (ch == '\n') {
        parser->line += 1;
        parser->column = 1;
    } else {
        parser->column += 1;
    }
    return ch;
}

static void parser_skip_space(Parser *parser) {
    for (;;) {
        uint32_t ch = parser_peek(parser);
        if (ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n') {
            parser_advance(parser);
            continue;
        }
        if (ch == ';') {
            while (parser_peek(parser) != '\n' && parser_peek(parser) != 0) {
                parser_advance(parser);
            }
            continue;
        }
        break;
    }
}

static char *parser_error(const Parser *parser, const char *message) {
    return format_string("%s at line %zu column %zu", message, parser->line, parser->column);
}

static int parser_is_delimiter(uint32_t ch) {
    return ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' || ch == '(' || ch == ')' || ch == ';';
}

static char *slice_chars(const uint32_t *chars, size_t start, size_t end, size_t count) {
    Buf out;
    buf_init(&out);
    size_t index = start;
    while (index < end && index < count) {
        uint32_t ch = chars[index];
        if (ch < 0x80) {
            buf_append_char(&out, (char)ch);
        } else if (ch < 0x800) {
            buf_append_char(&out, (char)(0xC0 | (ch >> 6)));
            buf_append_char(&out, (char)(0x80 | (ch & 0x3F)));
        } else if (ch < 0x10000) {
            buf_append_char(&out, (char)(0xE0 | (ch >> 12)));
            buf_append_char(&out, (char)(0x80 | ((ch >> 6) & 0x3F)));
            buf_append_char(&out, (char)(0x80 | (ch & 0x3F)));
        } else {
            buf_append_char(&out, (char)(0xF0 | (ch >> 18)));
            buf_append_char(&out, (char)(0x80 | ((ch >> 12) & 0x3F)));
            buf_append_char(&out, (char)(0x80 | ((ch >> 6) & 0x3F)));
            buf_append_char(&out, (char)(0x80 | (ch & 0x3F)));
        }
        index += 1;
    }
    return out.data;
}

static int parse_expr(Parser *parser, Sexp **out, char **error);

static int parse_text(Parser *parser, Sexp **out, char **error) {
    parser_advance(parser);
    Buf text;
    buf_init(&text);
    for (;;) {
        uint32_t ch = parser_advance(parser);
        if (ch == 0) {
            *error = parser_error(parser, "unterminated string literal");
            return 0;
        }
        if (ch == '"') {
            *out = make_str(text.data);
            return 1;
        }
        if (ch == '\\') {
            uint32_t escaped = parser_advance(parser);
            if (escaped == 0) {
                *error = parser_error(parser, "unterminated string literal");
                return 0;
            }
            if (escaped == 'n') {
                buf_append_char(&text, '\n');
            } else if (escaped == '\t') {
                buf_append_char(&text, '\t');
            } else if (escaped < 0x80) {
                buf_append_char(&text, (char)escaped);
            } else {
                uint32_t single = escaped;
                buf_append(&text, slice_chars(&single, 0, 1, 1));
            }
            continue;
        }
        if (ch < 0x80) {
            buf_append_char(&text, (char)ch);
        } else {
            uint32_t single = ch;
            buf_append(&text, slice_chars(&single, 0, 1, 1));
        }
    }
}

static int parse_number(Parser *parser, Sexp **out, char **error) {
    size_t start = parser->index;
    if (parser_peek(parser) == '-') {
        parser_advance(parser);
    }
    while (parser_peek(parser) >= '0' && parser_peek(parser) <= '9') {
        parser_advance(parser);
    }
    if (parser_is_delimiter(parser_peek(parser)) == 0 && parser_peek(parser) != 0) {
        while (parser_is_delimiter(parser_peek(parser)) == 0) {
            parser_advance(parser);
        }
        *error = parser_error(parser, "invalid number literal");
        return 0;
    }
    char *text = slice_chars(parser->chars, start, parser->index, parser->count);
    if (strcmp(text, "-") == 0) {
        *error = parser_error(parser, "invalid number literal");
        return 0;
    }
    errno = 0;
    char *end = NULL;
    long long value = strtoll(text, &end, 10);
    if (end == text || *end != '\0' || errno == ERANGE) {
        *error = parser_error(parser, "invalid number literal");
        return 0;
    }
    *out = make_num(value);
    return 1;
}

static int parse_symbol(Parser *parser, Sexp **out, char **error) {
    size_t start = parser->index;
    while (parser_is_delimiter(parser_peek(parser)) == 0 && parser_peek(parser) != 0) {
        parser_advance(parser);
    }
    char *text = slice_chars(parser->chars, start, parser->index, parser->count);
    if (strcmp(text, "#t") == 0) {
        *out = make_bool(1);
        return 1;
    }
    if (strcmp(text, "#f") == 0) {
        *out = make_bool(0);
        return 1;
    }
    if (text[0] == '\0') {
        *error = parser_error(parser, "empty symbol");
        return 0;
    }
    *out = make_sym(text);
    return 1;
}

static int parse_list(Parser *parser, Sexp **out, char **error) {
    SexpVec items;
    vec_init(&items);
    for (;;) {
        parser_skip_space(parser);
        uint32_t ch = parser_peek(parser);
        if (ch == 0) {
            *error = parser_error(parser, "expected ')' but reached end of input");
            return 0;
        }
        if (ch == ')') {
            parser_advance(parser);
            *out = make_list(items);
            return 1;
        }
        Sexp *item = NULL;
        if (parse_expr(parser, &item, error) == 0) {
            return 0;
        }
        vec_push(&items, item);
    }
}

static int parse_expr(Parser *parser, Sexp **out, char **error) {
    parser_skip_space(parser);
    uint32_t ch = parser_peek(parser);
    if (ch == 0) {
        *error = parser_error(parser, "unexpected end of input");
        return 0;
    }
    if (ch == '(') {
        parser_advance(parser);
        return parse_list(parser, out, error);
    }
    if (ch == ')') {
        *error = parser_error(parser, "unexpected ')'");
        return 0;
    }
    if (ch == '\'') {
        parser_advance(parser);
        Sexp *inner = NULL;
        if (parse_expr(parser, &inner, error) == 0) {
            return 0;
        }
        SexpVec pair;
        vec_init(&pair);
        vec_push(&pair, make_sym("quote"));
        vec_push(&pair, inner);
        *out = make_list(pair);
        return 1;
    }
    if (ch == '"') {
        return parse_text(parser, out, error);
    }
    if (ch >= '0' && ch <= '9') {
        return parse_number(parser, out, error);
    }
    if (ch == '-') {
        uint32_t next = parser_peek_next(parser);
        if (next >= '0' && next <= '9') {
            return parse_number(parser, out, error);
        }
        return parse_symbol(parser, out, error);
    }
    return parse_symbol(parser, out, error);
}

static int parse_all(const char *text, SexpVec *out, char **error) {
    Parser parser;
    parser_init(&parser, text);
    vec_init(out);
    for (;;) {
        parser_skip_space(&parser);
        if (parser_peek(&parser) == 0) {
            break;
        }
        Sexp *expr = NULL;
        if (parse_expr(&parser, &expr, error) == 0) {
            return 0;
        }
        vec_push(out, expr);
    }
    return 1;
}

// ---------------------------------------------------------------- evaluation

typedef struct {
    size_t expressions;
    size_t prints;
} Stats;

static void stats_init(Stats *stats) {
    stats->expressions = 0;
    stats->prints = 0;
}

static int eval(Sexp *expr, SexpVec *env, Stats *stats, Sexp **out, char **error);

static int expect_number(Sexp *expr, long long *out, char **error) {
    if (expr->kind == SEXP_NUM) {
        *out = expr->number;
        return 1;
    }
    *error = format_string("expected a number, found %s", display(expr));
    return 0;
}

static int eval_args(SexpVec *items, size_t start, SexpVec *env, Stats *stats, SexpVec *out, char **error) {
    vec_init(out);
    for (size_t index = start; index < items->length; index += 1) {
        Sexp *value = NULL;
        if (eval(items->items[index], env, stats, &value, error) == 0) {
            return 0;
        }
        vec_push(out, value);
    }
    return 1;
}

static int builtin_arithmetic(const char *name, SexpVec *args, Sexp **out, char **error) {
    if (args->length == 0) {
        *error = format_string("%s expects at least one argument", name);
        return 0;
    }
    long long total = 0;
    if (expect_number(args->items[0], &total, error) == 0) {
        return 0;
    }
    if (strcmp(name, "-") == 0 && args->length == 1) {
        *out = make_num(-total);
        return 1;
    }
    if (strcmp(name, "/") == 0 && args->length == 1) {
        if (total == 0) {
            *error = xstrdup("division by zero");
            return 0;
        }
        *out = make_num(1 / total);
        return 1;
    }
    for (size_t index = 1; index < args->length; index += 1) {
        long long value = 0;
        if (expect_number(args->items[index], &value, error) == 0) {
            return 0;
        }
        if (strcmp(name, "+") == 0) {
            total = total + value;
        } else if (strcmp(name, "-") == 0) {
            total = total - value;
        } else if (strcmp(name, "*") == 0) {
            total = total * value;
        } else {
            if (value == 0) {
                *error = xstrdup("division by zero");
                return 0;
            }
            total = total / value;
        }
    }
    *out = make_num(total);
    return 1;
}

static int builtin_compare(const char *name, SexpVec *args, Sexp **out, char **error) {
    if (args->length != 2) {
        *error = format_string("%s expects two arguments", name);
        return 0;
    }
    long long left = 0;
    long long right = 0;
    if (expect_number(args->items[0], &left, error) == 0) {
        return 0;
    }
    if (expect_number(args->items[1], &right, error) == 0) {
        return 0;
    }
    if (strcmp(name, "=") == 0) {
        *out = make_bool(left == right);
        return 1;
    }
    if (strcmp(name, "<") == 0) {
        *out = make_bool(left < right);
        return 1;
    }
    if (strcmp(name, ">") == 0) {
        *out = make_bool(left > right);
        return 1;
    }
    if (strcmp(name, "<=") == 0) {
        *out = make_bool(left <= right);
        return 1;
    }
    *out = make_bool(left >= right);
    return 1;
}

static int matches_number(const Sexp *expr) {
    return expr->kind == SEXP_NUM;
}

static int matches_string(const Sexp *expr) {
    return expr->kind == SEXP_STR;
}

static int matches_symbol(const Sexp *expr) {
    return expr->kind == SEXP_SYM;
}

static size_t utf8_char_count(const char *text) {
    size_t count = 0;
    const unsigned char *cursor = (const unsigned char *)text;
    while (*cursor != '\0') {
        if ((*cursor & 0xC0) != 0x80) {
            count += 1;
        }
        cursor += 1;
    }
    return count;
}

static int builtin_list(const char *name, SexpVec *args, Sexp **out, char **error) {
    if (strcmp(name, "list") == 0) {
        *out = make_list(vec_clone(args));
        return 1;
    }
    if (strcmp(name, "car") == 0 || strcmp(name, "cdr") == 0) {
        if (args->length != 1) {
            *error = format_string("%s expects one argument", name);
            return 0;
        }
        Sexp *value = args->items[0];
        if (value->kind != SEXP_LIST) {
            *error = format_string("%s expects a list", name);
            return 0;
        }
        if (value->list.length == 0) {
            *error = format_string("%s of an empty list", name);
            return 0;
        }
        if (strcmp(name, "car") == 0) {
            *out = sexp_clone(value->list.items[0]);
            return 1;
        }
        SexpVec rest;
        vec_init(&rest);
        for (size_t index = 1; index < value->list.length; index += 1) {
            vec_push(&rest, sexp_clone(value->list.items[index]));
        }
        *out = make_list(rest);
        return 1;
    }
    if (strcmp(name, "cons") == 0) {
        if (args->length != 2) {
            *error = xstrdup("cons expects two arguments");
            return 0;
        }
        SexpVec items;
        vec_init(&items);
        vec_push(&items, sexp_clone(args->items[0]));
        Sexp *tail = args->items[1];
        if (tail->kind == SEXP_LIST) {
            for (size_t index = 0; index < tail->list.length; index += 1) {
                vec_push(&items, sexp_clone(tail->list.items[index]));
            }
        } else {
            vec_push(&items, sexp_clone(tail));
        }
        *out = make_list(items);
        return 1;
    }
    if (strcmp(name, "length") == 0) {
        if (args->length != 1) {
            *error = xstrdup("length expects one argument");
            return 0;
        }
        Sexp *value = args->items[0];
        if (value->kind == SEXP_LIST) {
            *out = make_num((long long)value->list.length);
            return 1;
        }
        if (value->kind == SEXP_STR) {
            *out = make_num((long long)utf8_char_count(value->text));
            return 1;
        }
        *error = xstrdup("length expects a list or string");
        return 0;
    }
    if (strcmp(name, "null?") == 0) {
        if (args->length != 1) {
            *error = xstrdup("null? expects one argument");
            return 0;
        }
        Sexp *value = args->items[0];
        if (value->kind == SEXP_LIST) {
            *out = make_bool(value->list.length == 0);
            return 1;
        }
        *out = make_bool(0);
        return 1;
    }
    if (strcmp(name, "number?") == 0) {
        *out = make_bool(args->length == 1 && matches_number(args->items[0]));
        return 1;
    }
    if (strcmp(name, "string?") == 0) {
        *out = make_bool(args->length == 1 && matches_string(args->items[0]));
        return 1;
    }
    if (strcmp(name, "symbol?") == 0) {
        *out = make_bool(args->length == 1 && matches_symbol(args->items[0]));
        return 1;
    }
    if (strcmp(name, "not") == 0) {
        *out = make_bool(args->length == 1 && is_truthy(args->items[0]) == 0);
        return 1;
    }
    *error = format_string("unknown builtin %s", name);
    return 0;
}

static int apply_lambda(Sexp *callee, SexpVec *args, SexpVec *caller, Stats *stats, Sexp **out, char **error) {
    if (callee->kind != SEXP_LAMBDA) {
        *error = format_string("cannot call %s", display(callee));
        return 0;
    }
    if (args->length != callee->lambda.param_count) {
        *error = format_string("lambda expects %zu arguments, found %zu", callee->lambda.param_count, args->length);
        return 0;
    }
    SexpVec env = vec_clone(&callee->lambda.captured);
    for (size_t outer = 0; outer < caller->length; outer += 1) {
        char *outer_name = NULL;
        if (binding_name(caller->items[outer], &outer_name)) {
            int present = 0;
            if (env_lookup(&env, outer_name) != NULL) {
                present = 1;
            }
            if (present == 0) {
                vec_push(&env, sexp_clone(caller->items[outer]));
            }
        }
    }
    for (size_t index = 0; index < callee->lambda.param_count; index += 1) {
        env_push(&env, callee->lambda.params[index], sexp_clone(args->items[index]));
    }
    Sexp *result = make_bool(0);
    for (size_t body_index = 0; body_index < callee->lambda.body.length; body_index += 1) {
        Sexp *value = NULL;
        if (eval(callee->lambda.body.items[body_index], &env, stats, &value, error) == 0) {
            return 0;
        }
        result = value;
    }
    *out = result;
    return 1;
}

static int is_arithmetic_name(const char *name) {
    return strcmp(name, "+") == 0 || strcmp(name, "-") == 0 || strcmp(name, "*") == 0 || strcmp(name, "/") == 0;
}

static int is_compare_name(const char *name) {
    return strcmp(name, "=") == 0 || strcmp(name, "<") == 0 || strcmp(name, ">") == 0 ||
           strcmp(name, "<=") == 0 || strcmp(name, ">=") == 0;
}

static int is_list_builtin_name(const char *name) {
    return strcmp(name, "list") == 0 || strcmp(name, "car") == 0 || strcmp(name, "cdr") == 0 ||
           strcmp(name, "cons") == 0 || strcmp(name, "length") == 0 || strcmp(name, "null?") == 0 ||
           strcmp(name, "number?") == 0 || strcmp(name, "string?") == 0 || strcmp(name, "symbol?") == 0 ||
           strcmp(name, "not") == 0;
}

static int eval(Sexp *expr, SexpVec *env, Stats *stats, Sexp **out, char **error) {
    stats->expressions += 1;
    switch (expr->kind) {
    case SEXP_NUM:
    case SEXP_BOOL:
    case SEXP_STR:
    case SEXP_LAMBDA:
        *out = sexp_clone(expr);
        return 1;
    case SEXP_SYM: {
        Sexp *value = env_lookup(env, expr->text);
        if (value == NULL) {
            *error = format_string("unbound symbol: %s", expr->text);
            return 0;
        }
        *out = value;
        return 1;
    }
    case SEXP_LIST: {
        if (expr->list.length == 0) {
            SexpVec empty;
            vec_init(&empty);
            *out = make_list(empty);
            return 1;
        }
        Sexp *head = expr->list.items[0];
        if (head->kind == SEXP_SYM) {
            const char *special = head->text;
            if (strcmp(special, "quote") == 0) {
                if (expr->list.length != 2) {
                    *error = xstrdup("quote expects one argument");
                    return 0;
                }
                *out = sexp_clone(expr->list.items[1]);
                return 1;
            }
            if (strcmp(special, "if") == 0) {
                if (expr->list.length < 3 || expr->list.length > 4) {
                    *error = xstrdup("if expects two or three arguments");
                    return 0;
                }
                Sexp *condition = NULL;
                if (eval(expr->list.items[1], env, stats, &condition, error) == 0) {
                    return 0;
                }
                if (is_truthy(condition)) {
                    return eval(expr->list.items[2], env, stats, out, error);
                }
                if (expr->list.length == 4) {
                    return eval(expr->list.items[3], env, stats, out, error);
                }
                *out = make_bool(0);
                return 1;
            }
            if (strcmp(special, "define") == 0) {
                if (expr->list.length != 3) {
                    *error = xstrdup("define expects a name and a value");
                    return 0;
                }
                Sexp *name_expr = expr->list.items[1];
                if (name_expr->kind != SEXP_SYM) {
                    *error = xstrdup("define expects a symbol name");
                    return 0;
                }
                Sexp *value = NULL;
                if (eval(expr->list.items[2], env, stats, &value, error) == 0) {
                    return 0;
                }
                env_push(env, name_expr->text, value);
                *out = make_sym(name_expr->text);
                return 1;
            }
            if (strcmp(special, "set!") == 0) {
                if (expr->list.length != 3) {
                    *error = xstrdup("set! expects a name and a value");
                    return 0;
                }
                Sexp *name_expr = expr->list.items[1];
                if (name_expr->kind != SEXP_SYM) {
                    *error = xstrdup("set! expects a symbol name");
                    return 0;
                }
                Sexp *value = NULL;
                if (eval(expr->list.items[2], env, stats, &value, error) == 0) {
                    return 0;
                }
                if (env_set(env, name_expr->text, value) == 0) {
                    *error = format_string("unbound symbol: %s", name_expr->text);
                    return 0;
                }
                *out = make_sym(name_expr->text);
                return 1;
            }
            if (strcmp(special, "let") == 0) {
                if (expr->list.length < 3) {
                    *error = xstrdup("let expects bindings and a body");
                    return 0;
                }
                SexpVec nested = vec_clone(env);
                Sexp *bindings = expr->list.items[1];
                if (bindings->kind != SEXP_LIST) {
                    *error = xstrdup("let expects a list of bindings");
                    return 0;
                }
                for (size_t index = 0; index < bindings->list.length; index += 1) {
                    Sexp *pair = bindings->list.items[index];
                    if (pair->kind != SEXP_LIST) {
                        *error = xstrdup("let expects a list of bindings");
                        return 0;
                    }
                    if (pair->list.length != 2) {
                        *error = xstrdup("let binding must have a name and a value");
                        return 0;
                    }
                    Sexp *binding = pair->list.items[0];
                    if (binding->kind != SEXP_SYM) {
                        *error = xstrdup("let binding name must be a symbol");
                        return 0;
                    }
                    Sexp *value = NULL;
                    if (eval(pair->list.items[1], env, stats, &value, error) == 0) {
                        return 0;
                    }
                    env_push(&nested, binding->text, value);
                }
                Sexp *result = make_bool(0);
                for (size_t body_index = 2; body_index < expr->list.length; body_index += 1) {
                    Sexp *value = NULL;
                    if (eval(expr->list.items[body_index], &nested, stats, &value, error) == 0) {
                        return 0;
                    }
                    result = value;
                }
                *out = result;
                return 1;
            }
            if (strcmp(special, "lambda") == 0) {
                if (expr->list.length < 3) {
                    *error = xstrdup("lambda expects parameters and a body");
                    return 0;
                }
                Sexp *parameter_list = expr->list.items[1];
                if (parameter_list->kind != SEXP_LIST) {
                    *error = xstrdup("lambda expects a parameter list");
                    return 0;
                }
                char **params = xmalloc((parameter_list->list.length == 0 ? 1 : parameter_list->list.length) * sizeof(char *));
                size_t param_count = 0;
                for (size_t index = 0; index < parameter_list->list.length; index += 1) {
                    Sexp *param = parameter_list->list.items[index];
                    if (param->kind != SEXP_SYM) {
                        *error = xstrdup("lambda parameters must be symbols");
                        return 0;
                    }
                    params[param_count] = xstrdup(param->text);
                    param_count += 1;
                }
                SexpVec body;
                vec_init(&body);
                for (size_t index = 2; index < expr->list.length; index += 1) {
                    vec_push(&body, sexp_clone(expr->list.items[index]));
                }
                *out = make_lambda(params, param_count, body, vec_clone(env));
                return 1;
            }
            if (strcmp(special, "begin") == 0) {
                Sexp *result = make_bool(0);
                for (size_t index = 1; index < expr->list.length; index += 1) {
                    Sexp *value = NULL;
                    if (eval(expr->list.items[index], env, stats, &value, error) == 0) {
                        return 0;
                    }
                    result = value;
                }
                *out = result;
                return 1;
            }
            if (strcmp(special, "print") == 0) {
                for (size_t index = 1; index < expr->list.length; index += 1) {
                    Sexp *value = NULL;
                    if (eval(expr->list.items[index], env, stats, &value, error) == 0) {
                        return 0;
                    }
                    printf("%s\n", display(value));
                    stats->prints += 1;
                }
                *out = make_bool(1);
                return 1;
            }
            if (is_arithmetic_name(special)) {
                SexpVec args;
                if (eval_args(&expr->list, 1, env, stats, &args, error) == 0) {
                    return 0;
                }
                return builtin_arithmetic(special, &args, out, error);
            }
            if (is_compare_name(special)) {
                SexpVec args;
                if (eval_args(&expr->list, 1, env, stats, &args, error) == 0) {
                    return 0;
                }
                return builtin_compare(special, &args, out, error);
            }
            if (is_list_builtin_name(special)) {
                SexpVec args;
                if (eval_args(&expr->list, 1, env, stats, &args, error) == 0) {
                    return 0;
                }
                return builtin_list(special, &args, out, error);
            }
        }
        Sexp *callee = NULL;
        if (eval(expr->list.items[0], env, stats, &callee, error) == 0) {
            return 0;
        }
        SexpVec args;
        if (eval_args(&expr->list, 1, env, stats, &args, error) == 0) {
            return 0;
        }
        SexpVec caller = vec_clone(env);
        return apply_lambda(callee, &args, &caller, stats, out, error);
    }
    }
    *error = xstrdup("internal evaluator error");
    return 0;
}

// ------------------------------------------------------------ script driver

static const char *EMBEDDED_LINES[] = {
    "; embedded script for lisp.ls",
    "(define answer 42)",
    "(print answer)",
    "(define square (lambda (n) (* n n)))",
    "(print (square 12))",
    "(define fact (lambda (n) (if (<= n 1) 1 (* n (fact (- n 1))))))",
    "(print (fact 6))",
    "(define make-adder (lambda (n) (lambda (m) (+ n m))))",
    "(define add7 (make-adder 7))",
    "(print (add7 35))",
    "(print (list 1 2 3))",
    "(print (car (list 1 2 3)))",
    "(print (cdr (list 1 2 3)))",
    "(print (cons 0 (list 1 2)))",
    "(print (length (list \"a\" \"b\" \"c\")))",
    "(print (if (< 3 5) \"less\" \"more\"))",
    "(let ((x 5) (y 7)) (print (+ x y)))",
    "(print (quote (a b c)))",
    "(print 'sym)",
    "(set! answer (+ answer 1))",
    "(print answer)",
    "(define counter 0)",
    "(define bump (lambda () (begin (set! counter (+ counter 1)) counter)))",
    "(print (bump))",
    "(print (bump))",
    "(print (number? 12) (string? \"text\") (symbol? 'sym) (null? (list)))",
    "(print (not #f) (not #t))",
    "(print \"done\")",
};

static char *embedded_script(void) {
    Buf script;
    buf_init(&script);
    for (size_t index = 0; index < sizeof(EMBEDDED_LINES) / sizeof(EMBEDDED_LINES[0]); index += 1) {
        buf_append(&script, EMBEDDED_LINES[index]);
        buf_append_char(&script, '\n');
    }
    return script.data;
}

static size_t count_lines(const char *text) {
    size_t length = strlen(text);
    size_t newlines = 0;
    for (size_t index = 0; index < length; index += 1) {
        if (text[index] == '\n') {
            newlines += 1;
        }
    }
    if (length > 0 && text[length - 1] != '\n') {
        newlines += 1;
    }
    return newlines;
}

static int run_script(const char *text, Stats *stats, char **error) {
    SexpVec expressions;
    if (parse_all(text, &expressions, error) == 0) {
        return 0;
    }
    SexpVec env;
    vec_init(&env);
    stats_init(stats);
    for (size_t index = 0; index < expressions.length; index += 1) {
        Sexp *value = NULL;
        if (eval(expressions.items[index], &env, stats, &value, error) == 0) {
            return 0;
        }
    }
    return 1;
}

static void report_parse_error(const char *text) {
    SexpVec expressions;
    char *error = NULL;
    if (parse_all(text, &expressions, &error)) {
        printf("parse error demo: surprising success\n");
        return;
    }
    printf("parse error demo: %s\n", error);
}

static char *read_text_file(const char *path, char **error) {
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        *error = format_string("%s (os error %d)", strerror(errno), errno);
        return NULL;
    }
    Buf text;
    buf_init(&text);
    char chunk[4096];
    for (;;) {
        size_t read = fread(chunk, 1, sizeof(chunk), file);
        if (read == 0) {
            break;
        }
        buf_append_n(&text, chunk, read);
    }
    fclose(file);
    return text.data;
}

int main(int argc, char **argv) {
#ifdef _WIN32
    _setmode(_fileno(stdout), _O_BINARY);
#endif
    printf("lisp: a small Lisp interpreter\n");
    char *script = embedded_script();
    printf("embedded script lines: %zu\n", count_lines(script));
    Stats stats;
    char *error = NULL;
    if (run_script(script, &stats, &error)) {
        printf("expressions evaluated: %zu\n", stats.expressions);
        printf("values printed: %zu\n", stats.prints);
    } else {
        printf("script error: %s\n", error);
    }
    report_parse_error("(+ 1 2");
    report_parse_error("\"unterminated");
    if (argc > 1) {
        char *file_error = NULL;
        char *text = read_text_file(argv[1], &file_error);
        if (text == NULL) {
            printf("file read error: %s\n", file_error);
        } else {
            printf("file script lines: %zu\n", count_lines(text));
            Stats file_stats;
            char *run_error = NULL;
            if (run_script(text, &file_stats, &run_error)) {
                printf("expressions evaluated: %zu\n", file_stats.expressions);
                printf("values printed: %zu\n", file_stats.prints);
            } else {
                printf("file script error: %s\n", run_error);
            }
        }
    }
    return 0;
}
