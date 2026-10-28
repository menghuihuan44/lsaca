// binary_trees_refs: the same perfect binary tree of depth 16 (131,071 nodes,
// leaves carry consecutive payloads) and the same checksum as binary_trees, but
// the traversal borrows the tree instead of consuming it: sum_tree takes
// &Option<Box<Node>> and recurses with &current.left / &current.right, so no
// payload is deep-copied. Compare with binary_trees.ls, whose sum_tree takes
// Option<Box<Node>> by value and, in 1.4, explicitly clones the two child
// subtrees at each level - that deep copy is the measured cost of the by-value
// traversal.
struct Node {
    value: i64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

fn build(depth: i64, start: i64) -> Option<Box<Node>> {
    if depth == 0 {
        Some(Box::new(Node { value: start, left: None, right: None }))
    } else {
        let left = build(depth - 1, start);
        let right = build(depth - 1, start + (1 << (depth - 1)));
        Some(Box::new(Node { value: 0, left: left, right: right }))
    }
}

fn sum_tree(tree: &Option<Box<Node>>) -> i64 {
    match tree {
        None => 0,
        Some(boxed) => {
            let current: &Node = boxed;
            current.value + sum_tree(&current.left) + sum_tree(&current.right)
        }
    }
}

fn main() {
    let depth: i64 = 16;
    let tree = build(depth, 0);
    let total = sum_tree(&tree);
    println!("checksum={}", total);
}
