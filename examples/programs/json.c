// json.c -- byte-identical C counterpart of examples/programs/json.ls
//
// A JSON parser, validator and canonical serializer with the same embedded
// documents, statistics, error positions and output as the lsaca program.
//
//   gcc -std=c17 -O2 json.c -lm -o json
//   ./json
//   ./json some_file.json
//
// The only intentional difference is that codepoints above U+00FF cannot be
// decoded from \uXXXX escapes (same restriction as the lsaca program).

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <errno.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

/* ------------------------------------------------------------------ */
/* growable byte string                                                */
/* ------------------------------------------------------------------ */

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} Str;

static void oom(void) {
    fprintf(stderr, "out of memory\n");
    exit(1);
}

static void str_init(Str *s) {
    s->data = NULL;
    s->len = 0;
    s->cap = 0;
}

static void str_grow(Str *s, size_t extra) {
    size_t need = s->len + extra + 1;
    if (need <= s->cap) {
        return;
    }
    size_t cap = s->cap ? s->cap : 32;
    while (cap < need) {
        cap *= 2;
    }
    char *data = (char *)realloc(s->data, cap);
    if (!data) {
        oom();
    }
    s->data = data;
    s->cap = cap;
}

static void str_putc(Str *s, char c) {
    str_grow(s, 1);
    s->data[s->len++] = c;
    s->data[s->len] = '\0';
}

static void str_append(Str *s, const char *text) {
    size_t n = strlen(text);
    str_grow(s, n);
    memcpy(s->data + s->len, text, n);
    s->len += n;
    s->data[s->len] = '\0';
}

static void str_append_cp(Str *s, uint32_t cp) {
    char buf[4];
    size_t n;
    if (cp < 0x80) {
        buf[0] = (char)cp;
        n = 1;
    } else if (cp < 0x800) {
        buf[0] = (char)(0xC0 | (cp >> 6));
        buf[1] = (char)(0x80 | (cp & 0x3F));
        n = 2;
    } else if (cp < 0x10000) {
        buf[0] = (char)(0xE0 | (cp >> 12));
        buf[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
        buf[2] = (char)(0x80 | (cp & 0x3F));
        n = 3;
    } else {
        buf[0] = (char)(0xF0 | (cp >> 18));
        buf[1] = (char)(0x80 | ((cp >> 12) & 0x3F));
        buf[2] = (char)(0x80 | ((cp >> 6) & 0x3F));
        buf[3] = (char)(0x80 | (cp & 0x3F));
        n = 4;
    }
    str_grow(s, n);
    memcpy(s->data + s->len, buf, n);
    s->len += n;
    s->data[s->len] = '\0';
}

static void str_free(Str *s) {
    free(s->data);
    s->data = NULL;
    s->len = 0;
    s->cap = 0;
}

/* Decode one UTF-8 codepoint; invalid bytes are treated as themselves. */
static uint32_t utf8_decode(const char *data, size_t len, size_t *index) {
    size_t i = *index;
    unsigned char first = (unsigned char)data[i];
    if (first < 0x80) {
        *index = i + 1;
        return first;
    }
    size_t extra;
    uint32_t cp;
    if ((first & 0xE0) == 0xC0) {
        extra = 1;
        cp = first & 0x1F;
    } else if ((first & 0xF0) == 0xE0) {
        extra = 2;
        cp = first & 0x0F;
    } else if ((first & 0xF8) == 0xF0) {
        extra = 3;
        cp = first & 0x07;
    } else {
        *index = i + 1;
        return first;
    }
    if (i + extra >= len) {
        *index = i + 1;
        return first;
    }
    for (size_t k = 1; k <= extra; k++) {
        unsigned char next = (unsigned char)data[i + k];
        if ((next & 0xC0) != 0x80) {
            *index = i + 1;
            return first;
        }
        cp = (cp << 6) | (next & 0x3F);
    }
    *index = i + extra + 1;
    return cp;
}

/* ------------------------------------------------------------------ */
/* JSON values                                                         */
/* ------------------------------------------------------------------ */

typedef enum {
    VALUE_NULL,
    VALUE_BOOL,
    VALUE_NUMBER,
    VALUE_TEXT,
    VALUE_ARRAY,
    VALUE_OBJECT
} ValueKind;

typedef struct Value Value;

typedef struct {
    char *key;
    Value *value;
} Entry;

struct Value {
    ValueKind kind;
    bool flag;
    double number;
    char *text;
    Value **items;
    size_t item_count;
    size_t item_cap;
    Entry *entries;
    size_t entry_count;
    size_t entry_cap;
};

static Value *value_new(ValueKind kind) {
    Value *value = (Value *)calloc(1, sizeof(Value));
    if (!value) {
        oom();
    }
    value->kind = kind;
    return value;
}

static void array_push(Value *array, Value *item) {
    if (array->item_count == array->item_cap) {
        size_t cap = array->item_cap ? array->item_cap * 2 : 8;
        Value **items = (Value **)realloc(array->items, cap * sizeof(Value *));
        if (!items) {
            oom();
        }
        array->items = items;
        array->item_cap = cap;
    }
    array->items[array->item_count++] = item;
}

static void object_push(Value *object, char *key, Value *value) {
    if (object->entry_count == object->entry_cap) {
        size_t cap = object->entry_cap ? object->entry_cap * 2 : 8;
        Entry *entries = (Entry *)realloc(object->entries, cap * sizeof(Entry));
        if (!entries) {
            oom();
        }
        object->entries = entries;
        object->entry_cap = cap;
    }
    object->entries[object->entry_count].key = key;
    object->entries[object->entry_count].value = value;
    object->entry_count++;
}

/* ------------------------------------------------------------------ */
/* parser                                                              */
/* ------------------------------------------------------------------ */

typedef struct {
    const char *message;
    size_t line;
    size_t column;
} Error;

typedef struct {
    uint32_t *chars;
    size_t count;
    size_t index;
    size_t line;
    size_t column;
} Parser;

static bool parser_at_end(const Parser *parser) {
    return parser->index >= parser->count;
}

static int parser_peek(const Parser *parser) {
    if (parser->index >= parser->count) {
        return -1;
    }
    return (int)parser->chars[parser->index];
}

static int parser_advance(Parser *parser) {
    if (parser->index >= parser->count) {
        return -1;
    }
    uint32_t ch = parser->chars[parser->index];
    parser->index++;
    if (ch == (uint32_t)'\n') {
        parser->line++;
        parser->column = 1;
    } else {
        parser->column++;
    }
    return (int)ch;
}

static void parser_error(Parser *parser, Error *error, const char *message) {
    error->message = message;
    error->line = parser->line;
    error->column = parser->column;
}

static bool parser_next_is_digit(const Parser *parser) {
    int ch = parser_peek(parser);
    return ch >= (int)'0' && ch <= (int)'9';
}

static void parser_skip_whitespace(Parser *parser) {
    for (;;) {
        int ch = parser_peek(parser);
        if (ch < 0) {
            return;
        }
        if (ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r') {
            parser_advance(parser);
        } else {
            return;
        }
    }
}

static Value *parse_value(Parser *parser, Error *error);
static Value *parse_object(Parser *parser, Error *error);
static Value *parse_array(Parser *parser, Error *error);
static bool parse_string(Parser *parser, Str *out, Error *error);
static uint32_t parse_hex4(Parser *parser, bool *ok, Error *error);
static bool expect_word(Parser *parser, const char *word, Error *error);

static Value *parse_document(Parser *parser, Error *error) {
    parser_skip_whitespace(parser);
    Value *value = parse_value(parser, error);
    if (!value) {
        return NULL;
    }
    parser_skip_whitespace(parser);
    if (!parser_at_end(parser)) {
        parser_error(parser, error, "unexpected trailing characters");
        return NULL;
    }
    return value;
}

static bool expect_word(Parser *parser, const char *word, Error *error) {
    size_t length = strlen(word);
    for (size_t index = 0; index < length; index++) {
        int ch = parser_advance(parser);
        if (ch < 0) {
            parser_error(parser, error, "unexpected end of input");
            return false;
        }
        if (ch != (int)(unsigned char)word[index]) {
            parser_error(parser, error, "invalid literal");
            return false;
        }
    }
    return true;
}

static uint32_t parse_hex4(Parser *parser, bool *ok, Error *error) {
    uint32_t value = 0;
    for (size_t index = 0; index < 4; index++) {
        int ch = parser_advance(parser);
        if (ch < 0) {
            parser_error(parser, error, "unterminated unicode escape");
            *ok = false;
            return 0;
        }
        uint32_t digit;
        if (ch >= '0' && ch <= '9') {
            digit = (uint32_t)(ch - '0');
        } else if (ch >= 'a' && ch <= 'f') {
            digit = (uint32_t)(ch - 'a' + 10);
        } else if (ch >= 'A' && ch <= 'F') {
            digit = (uint32_t)(ch - 'A' + 10);
        } else {
            parser_error(parser, error, "invalid unicode escape digit");
            *ok = false;
            return 0;
        }
        value = value * 16 + digit;
    }
    *ok = true;
    return value;
}

static bool parse_string(Parser *parser, Str *out, Error *error) {
    str_init(out);
    parser_advance(parser);
    for (;;) {
        int ch = parser_advance(parser);
        if (ch < 0) {
            parser_error(parser, error, "unterminated string");
            return false;
        }
        if (ch == '"') {
            return true;
        }
        if (ch == '\\') {
            int escape = parser_advance(parser);
            if (escape < 0) {
                parser_error(parser, error, "unterminated escape sequence");
                return false;
            }
            switch (escape) {
                case '"': str_putc(out, '"'); break;
                case '\\': str_putc(out, '\\'); break;
                case '/': str_putc(out, '/'); break;
                case 'b': str_append_cp(out, 8); break;
                case 'f': str_append_cp(out, 12); break;
                case 'n': str_putc(out, '\n'); break;
                case 'r': str_putc(out, '\r'); break;
                case 't': str_putc(out, '\t'); break;
                case 'u': {
                    bool ok = false;
                    uint32_t code = parse_hex4(parser, &ok, error);
                    if (!ok) {
                        return false;
                    }
                    if (code > 255) {
                        parser_error(parser, error, "unicode escapes above U+00FF are not supported in this build");
                        return false;
                    }
                    str_append_cp(out, code);
                    break;
                }
                default:
                    parser_error(parser, error, "invalid escape sequence");
                    return false;
            }
        } else if (ch < 32) {
            parser_error(parser, error, "unescaped control character in string");
            return false;
        } else {
            str_append_cp(out, (uint32_t)ch);
        }
    }
}

static size_t collect_digits(Parser *parser, Str *text) {
    size_t count = 0;
    for (;;) {
        int ch = parser_peek(parser);
        if (ch < 0) {
            return count;
        }
        if (ch >= '0' && ch <= '9') {
            str_putc(text, (char)ch);
            parser_advance(parser);
            count++;
        } else {
            return count;
        }
    }
}

static bool collect_exponent_sign(Parser *parser, Str *text) {
    int ch = parser_peek(parser);
    if (ch == '+') {
        parser_advance(parser);
        str_putc(text, '+');
    } else if (ch == '-') {
        parser_advance(parser);
        str_putc(text, '-');
    }
    return collect_digits(parser, text) > 0;
}

static Value *parse_number(Parser *parser, Error *error) {
    Str text;
    str_init(&text);
    int ch = parser_peek(parser);
    if (ch == '-') {
        parser_advance(parser);
        str_putc(&text, '-');
    } else if (ch < 0) {
        parser_error(parser, error, "unexpected end of input");
        str_free(&text);
        return NULL;
    }
    ch = parser_peek(parser);
    if (ch == '0') {
        str_putc(&text, '0');
        parser_advance(parser);
        if (parser_next_is_digit(parser)) {
            parser_error(parser, error, "leading zeros are not allowed");
            str_free(&text);
            return NULL;
        }
    } else if (ch < 0) {
        parser_error(parser, error, "unexpected end of input");
        str_free(&text);
        return NULL;
    } else {
        size_t digits = collect_digits(parser, &text);
        if (digits == 0) {
            parser_error(parser, error, "invalid number");
            str_free(&text);
            return NULL;
        }
    }
    ch = parser_peek(parser);
    if (ch == '.') {
        parser_advance(parser);
        str_putc(&text, '.');
        size_t digits = collect_digits(parser, &text);
        if (digits == 0) {
            parser_error(parser, error, "missing fraction digits");
            str_free(&text);
            return NULL;
        }
    }
    ch = parser_peek(parser);
    if (ch == 'e' || ch == 'E') {
        parser_advance(parser);
        str_putc(&text, 'e');
        if (!collect_exponent_sign(parser, &text)) {
            parser_error(parser, error, "missing exponent digits");
            str_free(&text);
            return NULL;
        }
    }
    char *end = NULL;
    double number = strtod(text.data, &end);
    if (end == text.data || (end && *end != '\0')) {
        parser_error(parser, error, "invalid number");
        str_free(&text);
        return NULL;
    }
    str_free(&text);
    Value *value = value_new(VALUE_NUMBER);
    value->number = number;
    return value;
}

static int parser_peek_char(const Parser *parser) {
    return parser_peek(parser);
}

static Value *parse_value(Parser *parser, Error *error) {
    parser_skip_whitespace(parser);
    int ch = parser_peek_char(parser);
    if (ch < 0) {
        parser_error(parser, error, "unexpected end of input");
        return NULL;
    }
    if (ch == '{') {
        return parse_object(parser, error);
    }
    if (ch == '[') {
        return parse_array(parser, error);
    }
    if (ch == '"') {
        Str text;
        if (!parse_string(parser, &text, error)) {
            return NULL;
        }
        Value *value = value_new(VALUE_TEXT);
        value->text = text.data ? text.data : (char *)calloc(1, 1);
        return value;
    }
    if (ch == 't') {
        if (!expect_word(parser, "true", error)) {
            return NULL;
        }
        Value *value = value_new(VALUE_BOOL);
        value->flag = true;
        return value;
    }
    if (ch == 'f') {
        if (!expect_word(parser, "false", error)) {
            return NULL;
        }
        Value *value = value_new(VALUE_BOOL);
        value->flag = false;
        return value;
    }
    if (ch == 'n') {
        if (!expect_word(parser, "null", error)) {
            return NULL;
        }
        return value_new(VALUE_NULL);
    }
    if (ch == '-' || (ch >= '0' && ch <= '9')) {
        return parse_number(parser, error);
    }
    parser_error(parser, error, "unexpected character");
    return NULL;
}

static Value *parse_object(Parser *parser, Error *error) {
    parser_advance(parser);
    Value *object = value_new(VALUE_OBJECT);
    parser_skip_whitespace(parser);
    int ch = parser_peek_char(parser);
    if (ch == '}') {
        parser_advance(parser);
        return object;
    }
    if (ch < 0) {
        parser_error(parser, error, "unterminated object");
        return NULL;
    }
    for (;;) {
        parser_skip_whitespace(parser);
        ch = parser_peek_char(parser);
        Str key;
        if (ch == '"') {
            if (!parse_string(parser, &key, error)) {
                return NULL;
            }
        } else if (ch < 0) {
            parser_error(parser, error, "unterminated object");
            return NULL;
        } else {
            parser_error(parser, error, "expected a string object key");
            return NULL;
        }
        parser_skip_whitespace(parser);
        ch = parser_peek_char(parser);
        if (ch == ':') {
            parser_advance(parser);
        } else if (ch < 0) {
            parser_error(parser, error, "unterminated object");
            return NULL;
        } else {
            parser_error(parser, error, "expected ':' after object key");
            return NULL;
        }
        Value *value = parse_value(parser, error);
        if (!value) {
            return NULL;
        }
        bool duplicate = false;
        for (size_t index = 0; index < object->entry_count; index++) {
            if (strcmp(object->entries[index].key, key.data) == 0) {
                duplicate = true;
                break;
            }
        }
        if (duplicate) {
            parser_error(parser, error, "duplicate object key");
            return NULL;
        }
        object_push(object, key.data, value);
        parser_skip_whitespace(parser);
        ch = parser_peek_char(parser);
        if (ch == ',') {
            parser_advance(parser);
            parser_skip_whitespace(parser);
            ch = parser_peek_char(parser);
            if (ch == '}') {
                parser_error(parser, error, "trailing comma in object");
                return NULL;
            }
            if (ch < 0) {
                parser_error(parser, error, "unterminated object");
                return NULL;
            }
        } else if (ch == '}') {
            parser_advance(parser);
            break;
        } else if (ch < 0) {
            parser_error(parser, error, "unterminated object");
            return NULL;
        } else {
            parser_error(parser, error, "expected ',' or '}' after object value");
            return NULL;
        }
    }
    return object;
}

static Value *parse_array(Parser *parser, Error *error) {
    parser_advance(parser);
    Value *array = value_new(VALUE_ARRAY);
    parser_skip_whitespace(parser);
    int ch = parser_peek_char(parser);
    if (ch == ']') {
        parser_advance(parser);
        return array;
    }
    if (ch < 0) {
        parser_error(parser, error, "unterminated array");
        return NULL;
    }
    for (;;) {
        Value *value = parse_value(parser, error);
        if (!value) {
            return NULL;
        }
        array_push(array, value);
        parser_skip_whitespace(parser);
        ch = parser_peek_char(parser);
        if (ch == ',') {
            parser_advance(parser);
            parser_skip_whitespace(parser);
            ch = parser_peek_char(parser);
            if (ch == ']') {
                parser_error(parser, error, "trailing comma in array");
                return NULL;
            }
            if (ch < 0) {
                parser_error(parser, error, "unterminated array");
                return NULL;
            }
        } else if (ch == ']') {
            parser_advance(parser);
            break;
        } else if (ch < 0) {
            parser_error(parser, error, "unterminated array");
            return NULL;
        } else {
            parser_error(parser, error, "expected ',' or ']' after array element");
            return NULL;
        }
    }
    return array;
}

/* ------------------------------------------------------------------ */
/* statistics                                                          */
/* ------------------------------------------------------------------ */

typedef struct {
    long long nodes;
    long long objects;
    long long arrays;
    long long strings;
    long long numbers;
    long long bools;
    long long nulls;
    long long max_depth;
} Stats;

static void stats_walk(Stats *stats, const Value *value, long long depth) {
    stats->nodes++;
    if (depth > stats->max_depth) {
        stats->max_depth = depth;
    }
    switch (value->kind) {
        case VALUE_NULL:
            stats->nulls++;
            break;
        case VALUE_BOOL:
            stats->bools++;
            break;
        case VALUE_NUMBER:
            stats->numbers++;
            break;
        case VALUE_TEXT:
            stats->strings++;
            break;
        case VALUE_ARRAY:
            stats->arrays++;
            for (size_t index = 0; index < value->item_count; index++) {
                stats_walk(stats, value->items[index], depth + 1);
            }
            break;
        case VALUE_OBJECT:
            stats->objects++;
            for (size_t index = 0; index < value->entry_count; index++) {
                stats_walk(stats, value->entries[index].value, depth + 1);
            }
            break;
    }
}

/* ------------------------------------------------------------------ */
/* serializers                                                         */
/* ------------------------------------------------------------------ */

static char hex_digit(uint32_t value) {
    uint32_t digit = value % 16;
    if (digit < 10) {
        return (char)('0' + digit);
    }
    return (char)('A' + digit - 10);
}

static void escape_char_into(Str *out, uint32_t ch) {
    if (ch == '"') {
        str_append(out, "\\\"");
    } else if (ch == '\\') {
        str_append(out, "\\\\");
    } else if (ch == '\n') {
        str_append(out, "\\n");
    } else if (ch == '\r') {
        str_append(out, "\\r");
    } else if (ch == '\t') {
        str_append(out, "\\t");
    } else if (ch == 8) {
        str_append(out, "\\b");
    } else if (ch == 12) {
        str_append(out, "\\f");
    } else if (ch < 32 || ch == 127) {
        char buf[8];
        buf[0] = '\\';
        buf[1] = 'u';
        buf[2] = hex_digit(ch / 4096);
        buf[3] = hex_digit((ch / 256) % 16);
        buf[4] = hex_digit((ch / 16) % 16);
        buf[5] = hex_digit(ch % 16);
        buf[6] = '\0';
        str_append(out, buf);
    } else {
        str_append_cp(out, ch);
    }
}

static void escape_into(Str *out, const char *text) {
    str_putc(out, '"');
    size_t length = strlen(text);
    size_t index = 0;
    while (index < length) {
        uint32_t ch = utf8_decode(text, length, &index);
        escape_char_into(out, ch);
    }
    str_putc(out, '"');
}

static void format_number(double value, Str *out) {
    if (value == 0.0) {
        str_append(out, "0");
        return;
    }
    if (value <= 9000000000000000.0 && value >= -9000000000000000.0) {
        long long truncated = (long long)value;
        if ((double)truncated == value) {
            char buf[32];
            snprintf(buf, sizeof(buf), "%lld", truncated);
            str_append(out, buf);
            return;
        }
    }
    char buf[64];
    snprintf(buf, sizeof(buf), "%.6f", value);
    size_t length = strlen(buf);
    while (length > 0 && buf[length - 1] == '0') {
        length--;
    }
    if (length > 0 && buf[length - 1] == '.') {
        length--;
    }
    str_grow(out, length);
    memcpy(out->data + out->len, buf, length);
    out->len += length;
    out->data[out->len] = '\0';
}

static int compare_keys(const void *left, const void *right) {
    const char *a = *(const char *const *)left;
    const char *b = *(const char *const *)right;
    return strcmp(a, b);
}

static void write_value(Str *out, const Value *value) {
    switch (value->kind) {
        case VALUE_NULL:
            str_append(out, "null");
            break;
        case VALUE_BOOL:
            str_append(out, value->flag ? "true" : "false");
            break;
        case VALUE_NUMBER: {
            Str text;
            str_init(&text);
            format_number(value->number, &text);
            str_append(out, text.data);
            str_free(&text);
            break;
        }
        case VALUE_TEXT:
            escape_into(out, value->text);
            break;
        case VALUE_ARRAY:
            str_putc(out, '[');
            for (size_t index = 0; index < value->item_count; index++) {
                if (index > 0) {
                    str_putc(out, ',');
                }
                write_value(out, value->items[index]);
            }
            str_putc(out, ']');
            break;
        case VALUE_OBJECT: {
            size_t count = value->entry_count;
            const char **keys = (const char **)malloc(count * sizeof(char *));
            if (!keys) {
                oom();
            }
            for (size_t index = 0; index < count; index++) {
                keys[index] = value->entries[index].key;
            }
            qsort(keys, count, sizeof(char *), compare_keys);
            str_putc(out, '{');
            for (size_t key_index = 0; key_index < count; key_index++) {
                if (key_index > 0) {
                    str_putc(out, ',');
                }
                escape_into(out, keys[key_index]);
                str_putc(out, ':');
                for (size_t entry_index = 0; entry_index < count; entry_index++) {
                    if (strcmp(value->entries[entry_index].key, keys[key_index]) == 0) {
                        write_value(out, value->entries[entry_index].value);
                        break;
                    }
                }
            }
            str_putc(out, '}');
            free(keys);
            break;
        }
    }
}

static void indent_into(Str *out, size_t depth) {
    for (size_t count = 0; count < depth; count++) {
        str_append(out, "  ");
    }
}

static void write_pretty(Str *out, const Value *value, size_t depth) {
    switch (value->kind) {
        case VALUE_NULL:
            str_append(out, "null");
            break;
        case VALUE_BOOL:
            str_append(out, value->flag ? "true" : "false");
            break;
        case VALUE_NUMBER: {
            Str text;
            str_init(&text);
            format_number(value->number, &text);
            str_append(out, text.data);
            str_free(&text);
            break;
        }
        case VALUE_TEXT:
            escape_into(out, value->text);
            break;
        case VALUE_ARRAY:
            if (value->item_count == 0) {
                str_append(out, "[]");
                break;
            }
            str_append(out, "[\n");
            for (size_t index = 0; index < value->item_count; index++) {
                indent_into(out, depth + 1);
                write_pretty(out, value->items[index], depth + 1);
                if (index + 1 < value->item_count) {
                    str_putc(out, ',');
                }
                str_putc(out, '\n');
            }
            indent_into(out, depth);
            str_putc(out, ']');
            break;
        case VALUE_OBJECT: {
            size_t count = value->entry_count;
            if (count == 0) {
                str_append(out, "{}");
                break;
            }
            const char **keys = (const char **)malloc(count * sizeof(char *));
            if (!keys) {
                oom();
            }
            for (size_t index = 0; index < count; index++) {
                keys[index] = value->entries[index].key;
            }
            qsort(keys, count, sizeof(char *), compare_keys);
            str_append(out, "{\n");
            for (size_t key_index = 0; key_index < count; key_index++) {
                indent_into(out, depth + 1);
                escape_into(out, keys[key_index]);
                str_append(out, ": ");
                for (size_t entry_index = 0; entry_index < count; entry_index++) {
                    if (strcmp(value->entries[entry_index].key, keys[key_index]) == 0) {
                        write_pretty(out, value->entries[entry_index].value, depth + 1);
                        break;
                    }
                }
                if (key_index + 1 < count) {
                    str_putc(out, ',');
                }
                str_putc(out, '\n');
            }
            indent_into(out, depth);
            str_putc(out, '}');
            free(keys);
            break;
        }
    }
}

/* ------------------------------------------------------------------ */
/* driver                                                              */
/* ------------------------------------------------------------------ */

static void embedded_document(Str *text) {
    str_init(text);
    str_append(text, "{\n");
    str_append(text, "  \"name\": \"lsaca\",\n");
    str_append(text, "  \"version\": 2.2,\n");
    str_append(text, "  \"active\": true,\n");
    str_append(text, "  \"scores\": [1, 2.5, 3.75],\n");
    str_append(text, "  \"meta\": {\n");
    str_append(text, "    \"tags\": [\"lsaca\", \"json\", \"v2.2\"],\n");
    str_append(text, "    \"empty\": [],\n");
    str_append(text, "    \"nothing\": {}\n");
    str_append(text, "  },\n");
    str_append(text, "  \"unicode\": \"caf\\u00e9\",\n");
    str_append(text, "  \"escaped\": \"line\\nbreak\\ttab \\\"quoted\\\" back\\\\slash\",\n");
    str_append(text, "  \"final\": null\n");
    str_append(text, "}\n");
}

static char *read_file(const char *path, bool *ok, char **message) {
    FILE *file = fopen(path, "rb");
    if (!file) {
        int code = errno;
        const char *text = strerror(code);
        size_t length = strlen(text) + 64;
        char *buffer = (char *)malloc(length);
        if (!buffer) {
            oom();
        }
        snprintf(buffer, length, "%s (os error %d)", text, code);
        *ok = false;
        *message = buffer;
        return NULL;
    }
    fseek(file, 0, SEEK_END);
    long size = ftell(file);
    if (size < 0) {
        size = 0;
    }
    fseek(file, 0, SEEK_SET);
    char *buffer = (char *)malloc((size_t)size + 1);
    if (!buffer) {
        oom();
    }
    size_t read = fread(buffer, 1, (size_t)size, file);
    buffer[read] = '\0';
    fclose(file);
    *ok = true;
    *message = NULL;
    return buffer;
}

int main(int argc, char **argv) {
#ifdef _WIN32
    _setmode(_fileno(stdout), _O_BINARY);
#endif
    printf("json: parse, validate and canonicalize JSON\n");
    Str sources[8];
    embedded_document(&sources[0]);
    str_init(&sources[1]);
    str_append(&sources[1], "[1, 2, 3]");
    str_init(&sources[2]);
    str_append(&sources[2], "{\"z\": 1, \"a\": 2}");
    str_init(&sources[3]);
    str_append(&sources[3], "\"plain string\"");
    str_init(&sources[4]);
    str_append(&sources[4], "{\"a\": 1, \"a\": 2}");
    str_init(&sources[5]);
    str_append(&sources[5], "[1, 2,]");
    str_init(&sources[6]);
    str_append(&sources[6], "{\"key\": \"\\u4e2d\"}");
    str_init(&sources[7]);
    str_append(&sources[7], "{\"broken\": \"text}");
    printf("documents: %zu\n", sizeof(sources) / sizeof(sources[0]));
    long long accepted = 0;
    long long rejected = 0;
    for (size_t index = 0; index < sizeof(sources) / sizeof(sources[0]); index++) {
        size_t number = index + 1;
        /* parse, but reuse the printing of parse_and_report by peeking first:
           duplicate-key handling needs the same message stream position, so
           fold the accepted/rejected decision into one pass below. */
        size_t length = strlen(sources[index].data);
        size_t count = 0;
        size_t position = 0;
        while (position < length) {
            utf8_decode(sources[index].data, length, &position);
            count++;
        }
        uint32_t *chars = (uint32_t *)malloc(count * sizeof(uint32_t));
        if (!chars) {
            oom();
        }
        position = 0;
        for (size_t k = 0; k < count; k++) {
            chars[k] = utf8_decode(sources[index].data, length, &position);
        }
        Parser parser = { chars, count, 0, 1, 1 };
        Error error = { NULL, 0, 0 };
        Value *value = parse_document(&parser, &error);
        if (!value) {
            rejected++;
            printf("[%zu] rejected: %s at line %zu column %zu\n", number, error.message, error.line, error.column);
        } else {
            accepted++;
            printf("[%zu] parsed\n", number);
            Str canonical;
            str_init(&canonical);
            write_value(&canonical, value);
            printf("  canonical: %s\n", canonical.data);
            Stats stats = { 0, 0, 0, 0, 0, 0, 0, 0 };
            stats_walk(&stats, value, 1);
            printf("  stats: nodes=%lld objects=%lld arrays=%lld strings=%lld numbers=%lld bools=%lld nulls=%lld depth=%lld\n",
                   stats.nodes, stats.objects, stats.arrays, stats.strings, stats.numbers, stats.bools, stats.nulls, stats.max_depth);
            if (index == 0) {
                Str pretty;
                str_init(&pretty);
                write_pretty(&pretty, value, 0);
                printf("  pretty:\n");
                printf("%s\n", pretty.data);
                str_free(&pretty);
            }
            str_free(&canonical);
        }
        free(chars);
    }
    printf("summary: %lld accepted, %lld rejected\n", accepted, rejected);
    if (argc > 1) {
        const char *path = argv[1];
        printf("file: %s\n", path);
        bool ok = false;
        char *message = NULL;
        char *text = read_file(path, &ok, &message);
        if (!ok) {
            printf("file document: read error: %s\n", message);
            free(message);
        } else {
            size_t length = strlen(text);
            size_t count = 0;
            size_t position = 0;
            while (position < length) {
                utf8_decode(text, length, &position);
                count++;
            }
            uint32_t *chars = (uint32_t *)malloc(count * sizeof(uint32_t));
            if (!chars) {
                oom();
            }
            position = 0;
            for (size_t k = 0; k < count; k++) {
                chars[k] = utf8_decode(text, length, &position);
            }
            Parser parser = { chars, count, 0, 1, 1 };
            Error error = { NULL, 0, 0 };
            Value *value = parse_document(&parser, &error);
            if (!value) {
                printf("file document: rejected: %s at line %zu column %zu\n", error.message, error.line, error.column);
            } else {
                printf("file document: parsed\n");
                Str canonical;
                str_init(&canonical);
                write_value(&canonical, value);
                printf("%s\n", canonical.data);
                str_free(&canonical);
            }
            free(chars);
            free(text);
        }
    }
    return 0;
}
