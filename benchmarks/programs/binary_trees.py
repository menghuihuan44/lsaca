"""binary_trees: build a perfect binary tree (leaves carry consecutive
payloads), recursively sum the payloads.

Optional argument: depth (default 16, the medium scale).
"""

import sys

DEPTH = 16


class Node:
    __slots__ = ("value", "left", "right")

    def __init__(self, value, left, right):
        self.value = value
        self.left = left
        self.right = right


def build(depth, start):
    if depth == 0:
        return Node(start, None, None)
    return Node(
        0,
        build(depth - 1, start),
        build(depth - 1, start + (1 << (depth - 1))),
    )


def sum_tree(node):
    if node is None:
        return 0
    return node.value + sum_tree(node.left) + sum_tree(node.right)


def arg_int(args, index, fallback):
    if index < len(args):
        return int(args[index])
    return fallback


def main():
    depth = arg_int(sys.argv[1:], 0, DEPTH)
    print(f"checksum={sum_tree(build(depth, 0))}")


if __name__ == "__main__":
    main()
