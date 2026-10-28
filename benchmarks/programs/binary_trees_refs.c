/* binary_trees: build a perfect binary tree of depth 16 (131,071 nodes,
 * leaves carry consecutive payloads), recursively sum the payloads, then free
 * the tree. C uses malloc/free; the other languages use their own heap types. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define DEPTH 16

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

int main(void) {
    Node *tree = build(DEPTH, 0);
    int64_t total = sum_tree(tree);
    printf("checksum=%" PRId64 "\n", total);
    free_tree(tree);
    return 0;
}
