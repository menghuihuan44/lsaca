// wordstats.c -- byte-identical C counterpart of examples/programs/wordstats.ls.
//
// Build:
//   gcc -std=c17 -O2 -Wall -Wextra examples/programs/wordstats.c -o wordstats_c
//
// Run:
//   ./wordstats_c
//   ./wordstats_c data/sample.txt
//
// Words are runs of ASCII letters or digits (lowercased); every other
// character ends a word. The report and its order match the lsaca program.

#include <ctype.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

#define WORD_CAP 64

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} Buf;

typedef struct {
    char word[WORD_CAP];
    size_t count;
} Entry;

typedef struct {
    Entry *entries;
    size_t len;
    size_t cap;
} Table;

static void buf_init(Buf *buf);
static void buf_free(Buf *buf);
static void buf_reserve(Buf *buf, size_t extra);
static void buf_append_range(Buf *buf, const char *text, size_t length);
static void buf_append(Buf *buf, const char *text);
static void table_init(Table *table);
static void table_free(Table *table);
static void table_insert(Table *table, const char *word, size_t length);
static void flush_word(Table *table, char *current, size_t *current_len, size_t *words,
                       size_t *word_bytes);

static void fail(const char *message) {
    fprintf(stderr, "%s\n", message);
    exit(1);
}

static void buf_init(Buf *buf) {
    buf->cap = 64;
    buf->len = 0;
    buf->data = (char *)malloc(buf->cap);
    if (buf->data == NULL) {
        fail("out of memory");
    }
}

static void buf_free(Buf *buf) {
    free(buf->data);
    buf->data = NULL;
    buf->len = 0;
    buf->cap = 0;
}

static void buf_reserve(Buf *buf, size_t extra) {
    if (buf->len + extra + 1 <= buf->cap) {
        return;
    }
    size_t cap = buf->cap;
    while (cap < buf->len + extra + 1) {
        cap *= 2;
    }
    char *data = (char *)realloc(buf->data, cap);
    if (data == NULL) {
        fail("out of memory");
    }
    buf->data = data;
    buf->cap = cap;
}

static void buf_append_range(Buf *buf, const char *text, size_t length) {
    buf_reserve(buf, length);
    memcpy(buf->data + buf->len, text, length);
    buf->len += length;
    buf->data[buf->len] = 0;
}

static void buf_append(Buf *buf, const char *text) {
    buf_append_range(buf, text, strlen(text));
}

static void table_init(Table *table) {
    table->cap = 16;
    table->len = 0;
    table->entries = (Entry *)malloc(table->cap * sizeof(Entry));
    if (table->entries == NULL) {
        fail("out of memory");
    }
}

static void table_free(Table *table) {
    free(table->entries);
    table->entries = NULL;
    table->len = 0;
    table->cap = 0;
}

static void table_insert(Table *table, const char *word, size_t length) {
    size_t index;
    for (index = 0; index < table->len; index++) {
        if (strcmp(table->entries[index].word, word) == 0) {
            table->entries[index].count++;
            return;
        }
    }
    if (table->len == table->cap) {
        size_t cap = table->cap * 2;
        Entry *entries = (Entry *)realloc(table->entries, cap * sizeof(Entry));
        if (entries == NULL) {
            fail("out of memory");
        }
        table->entries = entries;
        table->cap = cap;
    }
    memcpy(table->entries[table->len].word, word, length);
    table->entries[table->len].word[length] = 0;
    table->entries[table->len].count = 1;
    table->len++;
}

static void flush_word(Table *table, char *current, size_t *current_len, size_t *words,
                       size_t *word_bytes) {
    if (*current_len == 0) {
        return;
    }
    current[*current_len] = 0;
    (*words)++;
    *word_bytes += *current_len;
    table_insert(table, current, *current_len);
    *current_len = 0;
}

static size_t bucket_of(size_t length) {
    if (length <= 2) {
        return 0;
    }
    if (length <= 4) {
        return 1;
    }
    if (length <= 6) {
        return 2;
    }
    if (length <= 8) {
        return 3;
    }
    if (length <= 10) {
        return 4;
    }
    return 5;
}

static int compare_entries(const void *left, const void *right) {
    const Entry *a = (const Entry *)left;
    const Entry *b = (const Entry *)right;
    if (a->count != b->count) {
        return a->count < b->count ? 1 : -1;
    }
    return strcmp(a->word, b->word);
}

static void print_report(const char *text, size_t length) {
    size_t lines = 0;
    size_t long_lines = 0;
    size_t segment = 0;
    size_t index;
    for (index = 0; index < length; index++) {
        if (text[index] == '\n') {
            size_t measured = segment;
            if (measured > 0 && text[index - 1] == '\r') {
                measured--;
            }
            lines++;
            if (measured > 40) {
                long_lines++;
            }
            segment = 0;
        } else {
            segment++;
        }
    }
    if (segment > 0) {
        size_t measured = segment;
        if (text[length - 1] == '\r') {
            measured--;
        }
        lines++;
        if (measured > 40) {
            long_lines++;
        }
    }

    Table table;
    char current[WORD_CAP];
    size_t current_len = 0;
    size_t words = 0;
    size_t word_bytes = 0;
    size_t lengths_cap = length + 1;
    size_t *lengths = (size_t *)malloc(lengths_cap * sizeof(size_t));
    size_t length_count = 0;
    if (lengths == NULL) {
        fail("out of memory");
    }
    table_init(&table);
    for (index = 0; index < length; index++) {
        unsigned char ch = (unsigned char)text[index];
        if (isalnum(ch)) {
            if (current_len + 1 >= WORD_CAP) {
                flush_word(&table, current, &current_len, &words, &word_bytes);
            }
            current[current_len] = (char)tolower(ch);
            current_len++;
        } else {
            if (current_len > 0) {
                lengths[length_count] = current_len;
                length_count++;
            }
            flush_word(&table, current, &current_len, &words, &word_bytes);
        }
    }
    if (current_len > 0) {
        lengths[length_count] = current_len;
        length_count++;
    }
    flush_word(&table, current, &current_len, &words, &word_bytes);

    qsort(table.entries, table.len, sizeof(Entry), compare_entries);

    const char *longest = "";
    size_t longest_len = 0;
    for (index = 0; index < table.len; index++) {
        size_t word_len = strlen(table.entries[index].word);
        if (word_len > longest_len
            || (word_len == longest_len && strcmp(table.entries[index].word, longest) < 0)) {
            longest = table.entries[index].word;
            longest_len = word_len;
        }
    }

    size_t buckets[6] = {0, 0, 0, 0, 0, 0};
    for (index = 0; index < length_count; index++) {
        buckets[bucket_of(lengths[index])]++;
    }

    printf("lines: %zu\n", lines);
    printf("long lines: %zu\n", long_lines);
    printf("bytes: %zu\n", length);
    printf("words: %zu\n", words);
    printf("unique words: %zu\n", table.len);
    printf("average word length: %zu\n", words == 0 ? 0 : word_bytes / words);
    printf("longest word: %s (%zu)\n", longest, longest_len);
    printf("top words:\n");
    for (index = 0; index < table.len && index < 5; index++) {
        printf("  %s %zu\n", table.entries[index].word, table.entries[index].count);
    }
    printf("histogram:\n");
    printf("  1-2: %zu\n", buckets[0]);
    printf("  3-4: %zu\n", buckets[1]);
    printf("  5-6: %zu\n", buckets[2]);
    printf("  7-8: %zu\n", buckets[3]);
    printf("  9-10: %zu\n", buckets[4]);
    printf("  11+: %zu\n", buckets[5]);
    printf("word bytes: %zu\n", word_bytes);

    free(lengths);
    table_free(&table);
}

static void embedded_text(Buf *text) {
    buf_append(text, "The quick brown fox jumps over the lazy dog.\n");
    buf_append(text, "A quick fox is quicker than a lazy dog, but the lazy dog sleeps.\n");
    buf_append(text, "Rust and C are systems languages; lsaca compiles to C.\n");
    buf_append(text, "Patterns, break values and iterators make the language pleasant.\n");
    buf_append(text, "The dog barks; the fox runs away!\n");
    buf_append(text, "The end.\n");
}

static int read_file(const char *path, Buf *out) {
    FILE *file = fopen(path, "rb");
    char chunk[4096];
    size_t got;
    if (file == NULL) {
        return 0;
    }
    while ((got = fread(chunk, 1, sizeof(chunk), file)) > 0) {
        buf_append_range(out, chunk, got);
    }
    fclose(file);
    return 1;
}

int main(int argc, char **argv) {
#ifdef _WIN32
    _setmode(_fileno(stdout), _O_BINARY);
#endif
    Buf text;
    buf_init(&text);
    embedded_text(&text);
    printf("word statistics\n");
    print_report(text.data, text.len);
    buf_free(&text);
    if (argc > 1) {
        const char *path = argv[1];
        Buf file_text;
        printf("file: %s\n", path);
        buf_init(&file_text);
        if (!read_file(path, &file_text)) {
            printf("file read error: %s (os error %d)\n", strerror(errno), errno);
        } else {
            print_report(file_text.data, file_text.len);
        }
        buf_free(&file_text);
    }
    return 0;
}
