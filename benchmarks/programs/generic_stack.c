// generic_stack: the same stack interface instantiated for two element types,
// i64 and a heap-allocated string. Push one million integers, pop them all into
// a sum; push one thousand small strings, pop them all into a total byte
// length. C has no generics, so the two instantiations are written out.
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    int64_t *items;
    size_t len;
    size_t capacity;
} IntStack;

static void int_stack_init(IntStack *stack) {
    stack->items = (int64_t *)malloc(sizeof(int64_t) * 16);
    if (stack->items == NULL) {
        fprintf(stderr, "out of memory\n");
        exit(1);
    }
    stack->len = 0;
    stack->capacity = 16;
}

static void int_stack_push(IntStack *stack, int64_t item) {
    if (stack->len == stack->capacity) {
        size_t capacity = stack->capacity * 2;
        int64_t *items = (int64_t *)realloc(stack->items, sizeof(int64_t) * capacity);
        if (items == NULL) {
            fprintf(stderr, "out of memory\n");
            exit(1);
        }
        stack->items = items;
        stack->capacity = capacity;
    }
    stack->items[stack->len] = item;
    stack->len++;
}

static int64_t int_stack_pop(IntStack *stack) {
    stack->len--;
    return stack->items[stack->len];
}

typedef struct {
    char **items;
    size_t len;
    size_t capacity;
} StrStack;

static void str_stack_init(StrStack *stack) {
    stack->items = (char **)malloc(sizeof(char *) * 16);
    if (stack->items == NULL) {
        fprintf(stderr, "out of memory\n");
        exit(1);
    }
    stack->len = 0;
    stack->capacity = 16;
}

static void str_stack_push(StrStack *stack, const char *text) {
    if (stack->len == stack->capacity) {
        size_t capacity = stack->capacity * 2;
        char **items = (char **)realloc(stack->items, sizeof(char *) * capacity);
        if (items == NULL) {
            fprintf(stderr, "out of memory\n");
            exit(1);
        }
        stack->items = items;
        stack->capacity = capacity;
    }
    size_t size = strlen(text) + 1;
    char *copy = (char *)malloc(size);
    if (copy == NULL) {
        fprintf(stderr, "out of memory\n");
        exit(1);
    }
    memcpy(copy, text, size);
    stack->items[stack->len] = copy;
    stack->len++;
}

static char *str_stack_pop(StrStack *stack) {
    stack->len--;
    return stack->items[stack->len];
}

int main(void) {
    const int64_t n = 1000000;
    IntStack numbers;
    int_stack_init(&numbers);
    for (int64_t i = 0; i < n; i++) {
        int_stack_push(&numbers, i);
    }
    int64_t total = 0;
    while (numbers.len > 0) {
        total += int_stack_pop(&numbers);
    }
    free(numbers.items);

    StrStack words;
    str_stack_init(&words);
    char word[32];
    for (int64_t j = 0; j < 1000; j++) {
        snprintf(word, sizeof word, "item%lld", (long long)(j % 100));
        str_stack_push(&words, word);
    }
    int64_t char_total = 0;
    while (words.len > 0) {
        char *text = str_stack_pop(&words);
        char_total += (int64_t)strlen(text);
        free(text);
    }
    free(words.items);

    printf("checksum=%lld,%lld\n", (long long)total, (long long)char_total);
    return 0;
}
