// binary_trees: build a perfect binary tree of depth 16 (131,071 nodes, leaves
// carry consecutive payloads) through recursive Option<Box<Node>> fields, then
// recursively sum the payloads. Box values are deep-copied by lsaca value
// semantics, so build/traverse copies the tree at each level; that cost is part
// of what this workload measures.
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

fn sum_tree(node: Option<Box<Node>>) -> i64 {
    match node {
        None => 0,
        Some(boxed) => {
            let inner = boxed.get();
            inner.value + sum_tree(inner.left) + sum_tree(inner.right)
        }
    }
}

fn main() {
    let depth: i64 = 16;
    let tree = build(depth, 0);
    let total = sum_tree(tree);
    println!("checksum={}", total);
}
