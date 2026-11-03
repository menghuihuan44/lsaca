/* binary_trees_refs: identical tree and checksum to binary_trees; the lsaca
 * version differs only by traversing through borrowed references, so the C
 * program is the same as binary_trees.c.
 * Optional argument: depth (default 16, the medium scale). */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int64_t arg_i64(int argc, char **argv, int index, int64_t fallback) {
    if (index < argc - 1) {
        return strtoll(argv[index + 1], NULL, 10);
    }
    return fallback;
}

typedef struct Node {
    int64_t value;
    struct Node *left;
    struct Node *right;
} Node;

static Node *build(int64_t depth, int64_t start) {
    Node *node = (Node *)malloc(sizeof(Node));
    if (node == NULL) {
        fprintf(stderr, "allocation failed\n");
        exit(1);
    }
    if (depth == 0) {
        node->value = start;
        node->left = NULL;
        node->right = NULL;
    } else {
        node->value = 0;
        node->left = build(depth - 1, start);
        node->right = build(depth - 1, start + ((int64_t)1 << (depth - 1)));
    }
    return node;
}

static int64_t sum_tree(const Node *node) {
    if (node == NULL) {
        return 0;
    }
    return node->value + sum_tree(node->left) + sum_tree(node->right);
}

static void free_tree(Node *node) {
    if (node == NULL) {
        return;
    }
    free_tree(node->left);
    free_tree(node->right);
    free(node);
}

int main(int argc, char **argv) {
    const int64_t depth = arg_i64(argc, argv, 0, 16);
    Node *tree = build(depth, 0);
    int64_t total = sum_tree(tree);
    printf("checksum=%" PRId64 "\n", total);
    free_tree(tree);
    return 0;
}