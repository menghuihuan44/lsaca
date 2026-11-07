// markdown.c -- byte-identical C counterpart of examples/programs/markdown.ls.
//
// Build:
//   gcc -std=c17 -O2 -Wall -Wextra examples/programs/markdown.c -lm -o markdown_c
//
// Run:
//   ./markdown_c
//   ./markdown_c document.md
//
// The supported Markdown subset and the emitted HTML are identical to the
// lsaca program: headings, paragraphs, unordered lists, block quotes, fenced
// code blocks, inline **bold**, *italic*, `code`, [links](url) and escapes.

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} Buf;

typedef struct {
    size_t headings;
    size_t paragraphs;
    size_t lists;
    size_t items;
    size_t quotes;
    size_t code_blocks;
} Stats;

typedef struct {
    const char *ptr;
    size_t len;
} Line;

static void buf_init(Buf *buf);
static void buf_free(Buf *buf);
static void buf_reserve(Buf *buf, size_t extra);
static void buf_push(Buf *buf, char ch);
static void buf_append(Buf *buf, const char *text);
static void buf_append_range(Buf *buf, const char *text, size_t length);

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
    buf->data[0] = '\0';
}

static void buf_free(Buf *buf) {
    free(buf->data);
    buf->data = NULL;
    buf->len = 0;
    buf->cap = 0;
}

static void buf_reserve(Buf *buf, size_t extra) {
    size_t need = buf->len + extra + 1;
    size_t cap = buf->cap;
    char *data;
    if (need <= buf->cap) {
        return;
    }
    while (cap < need) {
        cap *= 2;
    }
    data = (char *)realloc(buf->data, cap);
    if (data == NULL) {
        fail("out of memory");
    }
    buf->data = data;
    buf->cap = cap;
}

static void buf_push(Buf *buf, char ch) {
    buf_reserve(buf, 1);
    buf->data[buf->len] = ch;
    buf->len += 1;
    buf->data[buf->len] = '\0';
}

static void buf_append(Buf *buf, const char *text) {
    buf_append_range(buf, text, strlen(text));
}

static void buf_append_range(Buf *buf, const char *text, size_t length) {
    buf_reserve(buf, length);
    memcpy(buf->data + buf->len, text, length);
    buf->len += length;
    buf->data[buf->len] = '\0';
}

static void push_escaped(Buf *out, char ch) {
    if (ch == '&') {
        buf_append(out, "&amp;");
    } else if (ch == '<') {
        buf_append(out, "&lt;");
    } else if (ch == '>') {
        buf_append(out, "&gt;");
    } else if (ch == '"') {
        buf_append(out, "&quot;");
    } else {
        buf_push(out, ch);
    }
}

static void escape_text(Buf *out, const char *text, size_t length) {
    size_t index = 0;
    while (index < length) {
        push_escaped(out, text[index]);
        index += 1;
    }
}

static size_t find_char(const char *text, size_t length, size_t from, char needle) {
    size_t index = from;
    while (index < length) {
        if (text[index] == needle) {
            return index;
        }
        index += 1;
    }
    return length;
}

static size_t find_pair(const char *text, size_t length, size_t from) {
    size_t index = from;
    while (index + 1 < length) {
        if (text[index] == '*' && text[index + 1] == '*') {
            return index;
        }
        index += 1;
    }
    return length;
}

static int starts_with(const char *text, size_t length, const char *prefix) {
    size_t prefix_len = strlen(prefix);
    if (prefix_len > length) {
        return 0;
    }
    return memcmp(text, prefix, prefix_len) == 0;
}

static void render_inline(Buf *out, const char *text, size_t length) {
    size_t index = 0;
    while (index < length) {
        char ch = text[index];
        if (ch == '\\') {
            if (index + 1 < length) {
                push_escaped(out, text[index + 1]);
                index += 2;
            } else {
                push_escaped(out, ch);
                index += 1;
            }
        } else if (ch == '`') {
            size_t close = find_char(text, length, index + 1, '`');
            if (close < length) {
                buf_append(out, "<code>");
                escape_text(out, text + index + 1, close - index - 1);
                buf_append(out, "</code>");
                index = close + 1;
            } else {
                push_escaped(out, ch);
                index += 1;
            }
        } else if (ch == '*') {
            if (index + 1 < length && text[index + 1] == '*') {
                size_t close = find_pair(text, length, index + 2);
                if (close < length) {
                    buf_append(out, "<strong>");
                    render_inline(out, text + index + 2, close - index - 2);
                    buf_append(out, "</strong>");
                    index = close + 2;
                } else {
                    push_escaped(out, ch);
                    index += 1;
                }
            } else {
                size_t close = find_char(text, length, index + 1, '*');
                if (close < length && close > index + 1) {
                    buf_append(out, "<em>");
                    render_inline(out, text + index + 1, close - index - 1);
                    buf_append(out, "</em>");
                    index = close + 1;
                } else {
                    push_escaped(out, ch);
                    index += 1;
                }
            }
        } else if (ch == '[') {
            size_t close = find_char(text, length, index + 1, ']');
            if (close < length && close + 1 < length && text[close + 1] == '(') {
                size_t end = find_char(text, length, close + 2, ')');
                if (end < length) {
                    buf_append(out, "<a href=\"");
                    escape_text(out, text + close + 2, end - close - 2);
                    buf_append(out, "\">");
                    render_inline(out, text + index + 1, close - index - 1);
                    buf_append(out, "</a>");
                    index = end + 1;
                } else {
                    push_escaped(out, ch);
                    index += 1;
                }
            } else {
                push_escaped(out, ch);
                index += 1;
            }
        } else {
            push_escaped(out, ch);
            index += 1;
        }
    }
}

static size_t heading_level(const char *text, size_t length) {
    size_t level = 0;
    while (level < length && text[level] == '#') {
        level += 1;
    }
    if (level >= 1 && level <= 6 && level < length && text[level] == ' ') {
        return level;
    }
    return 0;
}

static int is_trim_char(char ch) {
    return ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' || ch == '\v' || ch == '\f';
}

static void trim_span(const char *text, size_t length, const char **start_out, size_t *len_out) {
    size_t start = 0;
    size_t end = length;
    while (start < end && is_trim_char(text[start])) {
        start += 1;
    }
    while (end > start && is_trim_char(text[end - 1])) {
        end -= 1;
    }
    *start_out = text + start;
    *len_out = end - start;
}

static int is_block_start(const char *text, size_t length) {
    if (heading_level(text, length) > 0) {
        return 1;
    }
    if (starts_with(text, length, "- ")) {
        return 1;
    }
    if (length >= 1 && text[0] == '>') {
        return 1;
    }
    if (starts_with(text, length, "```")) {
        return 1;
    }
    return 0;
}

static Line *split_lines(const char *document, size_t document_len, size_t *count_out) {
    size_t capacity = 8;
    size_t count = 0;
    size_t pos = 0;
    Line *lines = (Line *)malloc(capacity * sizeof(Line));
    if (lines == NULL) {
        fail("out of memory");
    }
    while (pos < document_len) {
        size_t start = pos;
        size_t length;
        while (pos < document_len && document[pos] != '\n') {
            pos += 1;
        }
        length = pos - start;
        if (pos < document_len) {
            pos += 1;
        }
        if (count == capacity) {
            Line *grown;
            capacity *= 2;
            grown = (Line *)realloc(lines, capacity * sizeof(Line));
            if (grown == NULL) {
                fail("out of memory");
            }
            lines = grown;
        }
        lines[count].ptr = document + start;
        lines[count].len = length;
        count += 1;
    }
    *count_out = count;
    return lines;
}

static size_t count_lines_of(const char *document, size_t document_len) {
    size_t count = 0;
    size_t pos = 0;
    while (pos < document_len) {
        while (pos < document_len && document[pos] != '\n') {
            pos += 1;
        }
        if (pos < document_len) {
            pos += 1;
        }
        count += 1;
    }
    return count;
}

static void render_document(Buf *out, Stats *stats, const char *document, size_t document_len) {
    size_t count = 0;
    size_t index = 0;
    Line *lines = split_lines(document, document_len, &count);
    while (index < count) {
        const char *trimmed = lines[index].ptr;
        size_t trimmed_len = lines[index].len;
        trim_span(lines[index].ptr, lines[index].len, &trimmed, &trimmed_len);
        if (trimmed_len == 0) {
            index += 1;
        } else if (starts_with(trimmed, trimmed_len, "```")) {
            stats->code_blocks += 1;
            buf_append(out, "<pre><code>");
            index += 1;
            {
                int first = 1;
                while (index < count) {
                    const char *code_trimmed = lines[index].ptr;
                    size_t code_trimmed_len = lines[index].len;
                    trim_span(lines[index].ptr, lines[index].len, &code_trimmed, &code_trimmed_len);
                    if (starts_with(code_trimmed, code_trimmed_len, "```")) {
                        index += 1;
                        break;
                    }
                    if (!first) {
                        buf_push(out, '\n');
                    }
                    escape_text(out, lines[index].ptr, lines[index].len);
                    first = 0;
                    index += 1;
                }
            }
            buf_append(out, "</code></pre>\n");
        } else if (heading_level(trimmed, trimmed_len) > 0) {
            size_t level = heading_level(trimmed, trimmed_len);
            const char *display = trimmed + level + 1;
            size_t display_len = trimmed_len - level - 1;
            char tag[32];
            stats->headings += 1;
            trim_span(display, display_len, &display, &display_len);
            snprintf(tag, sizeof(tag), "<h%zu>", level);
            buf_append(out, tag);
            render_inline(out, display, display_len);
            snprintf(tag, sizeof(tag), "</h%zu>\n", level);
            buf_append(out, tag);
            index += 1;
        } else if (starts_with(trimmed, trimmed_len, "- ")) {
            stats->lists += 1;
            buf_append(out, "<ul>\n");
            while (index < count) {
                const char *item = lines[index].ptr;
                size_t item_len = lines[index].len;
                const char *display;
                size_t display_len;
                trim_span(lines[index].ptr, lines[index].len, &item, &item_len);
                if (!starts_with(item, item_len, "- ")) {
                    break;
                }
                trim_span(item + 2, item_len - 2, &display, &display_len);
                buf_append(out, "<li>");
                render_inline(out, display, display_len);
                buf_append(out, "</li>\n");
                stats->items += 1;
                index += 1;
            }
            buf_append(out, "</ul>\n");
        } else if (trimmed_len >= 1 && trimmed[0] == '>') {
            Buf quoted;
            stats->quotes += 1;
            buf_init(&quoted);
            while (index < count) {
                const char *item = lines[index].ptr;
                size_t item_len = lines[index].len;
                size_t start = 1;
                const char *display;
                size_t display_len;
                trim_span(lines[index].ptr, lines[index].len, &item, &item_len);
                if (!(item_len >= 1 && item[0] == '>')) {
                    break;
                }
                if (start < item_len && item[start] == ' ') {
                    start += 1;
                }
                trim_span(item + start, item_len - start, &display, &display_len);
                if (quoted.len > 0) {
                    buf_push(&quoted, ' ');
                }
                buf_append_range(&quoted, display, display_len);
                index += 1;
            }
            buf_append(out, "<blockquote>\n<p>");
            render_inline(out, quoted.data, quoted.len);
            buf_append(out, "</p>\n</blockquote>\n");
            buf_free(&quoted);
        } else {
            Buf paragraph;
            stats->paragraphs += 1;
            buf_init(&paragraph);
            while (index < count) {
                const char *item = lines[index].ptr;
                size_t item_len = lines[index].len;
                trim_span(lines[index].ptr, lines[index].len, &item, &item_len);
                if (item_len == 0) {
                    break;
                }
                if (is_block_start(item, item_len)) {
                    break;
                }
                if (paragraph.len > 0) {
                    buf_push(&paragraph, ' ');
                }
                buf_append_range(&paragraph, item, item_len);
                index += 1;
            }
            buf_append(out, "<p>");
            render_inline(out, paragraph.data, paragraph.len);
            buf_append(out, "</p>\n");
            buf_free(&paragraph);
        }
    }
    free(lines);
}

static void embedded_document(Buf *text) {
    buf_append(text, "# Markdown demo\n");
    buf_append(text, "\n");
    buf_append(text, "markdown.ls converts a small Markdown subset to HTML. It handles **bold**\n");
    buf_append(text, "text, *italic* text, `inline code` and links such as\n");
    buf_append(text, "[the lsaca repository](https://example.com/lsaca?lang=lsaca&tab=docs).\n");
    buf_append(text, "\n");
    buf_append(text, "Escapes keep markup literal: \\*not italic\\*, \\`not code\\`, \\# no heading.\n");
    buf_append(text, "\n");
    buf_append(text, "## Lists, quotes and code\n");
    buf_append(text, "\n");
    buf_append(text, "- first item\n");
    buf_append(text, "- second item with **bold** and *emphasis*\n");
    buf_append(text, "- third item with `x = 1` and a [link](https://example.com/list)\n");
    buf_append(text, "\n");
    buf_append(text, "> quoted line one\n");
    buf_append(text, "> quoted line two with **bold** and `code`\n");
    buf_append(text, "> and a [link](https://example.com/quote)\n");
    buf_append(text, "\n");
    buf_append(text, "```lsaca\n");
    buf_append(text, "fn main() {\n");
    buf_append(text, "    println!(\"hello from a fenced block <tag> & more\");\n");
    buf_append(text, "}\n");
    buf_append(text, "```\n");
    buf_append(text, "\n");
    buf_append(text, "Trailing paragraph with unicode: café, naïve, and 5 < 6 & 7 > 6.\n");
}

static void print_report(const Stats *stats) {
    printf("blocks: headings=%zu paragraphs=%zu lists=%zu items=%zu quotes=%zu code=%zu\n",
           stats->headings, stats->paragraphs, stats->lists, stats->items, stats->quotes,
           stats->code_blocks);
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
    Buf document;
    Buf html;
    Stats stats;

    buf_init(&document);
    embedded_document(&document);
    printf("markdown: convert a Markdown subset to HTML\n");
    memset(&stats, 0, sizeof(stats));
    buf_init(&html);
    render_document(&html, &stats, document.data, document.len);
    printf("input lines: %zu\n", count_lines_of(document.data, document.len));
    printf("output bytes: %zu\n", html.len);
    print_report(&stats);
    fwrite(html.data, 1, html.len, stdout);
    printf("\n");
    if (argc > 1) {
        const char *path = argv[1];
        Buf file_text;
        printf("file: %s\n", path);
        buf_init(&file_text);
        if (!read_file(path, &file_text)) {
            printf("file read error: %s (os error %d)\n", strerror(errno), errno);
        } else {
            Buf file_html;
            Stats file_stats;
            memset(&file_stats, 0, sizeof(file_stats));
            buf_init(&file_html);
            render_document(&file_html, &file_stats, file_text.data, file_text.len);
            printf("file input lines: %zu\n", count_lines_of(file_text.data, file_text.len));
            printf("file output bytes: %zu\n", file_html.len);
            print_report(&file_stats);
            fwrite(file_html.data, 1, file_html.len, stdout);
            printf("\n");
            buf_free(&file_html);
        }
        buf_free(&file_text);
    }
    buf_free(&html);
    buf_free(&document);
    return 0;
}
