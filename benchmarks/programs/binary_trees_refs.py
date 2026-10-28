"""binary_trees: build a perfect binary tree of depth 16 (131,071 nodes,
leaves carry consecutive payloads), recursively sum the payloads."""

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


def main():
    print(f"checksum={sum_tree(build(DEPTH, 0))}")


if __name__ == "__main__":
    main()
