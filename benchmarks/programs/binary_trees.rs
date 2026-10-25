// binary_trees: build a perfect binary tree of depth 16 (131,071 nodes, leaves
// carry consecutive payloads), recursively sum the payloads. Rust nodes are
// Box-allocated and traversed by moving out of the boxes (no payload copies).
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
        Some(Box::new(Node { value: 0, left, right }))
    }
}

fn sum_tree(node: Option<Box<Node>>) -> i64 {
    match node {
        None => 0,
        Some(boxed) => {
            let Node { value, left, right } = *boxed;
            value + sum_tree(left) + sum_tree(right)
        }
    }
}

fn main() {
    let tree = build(16, 0);
    println!("checksum={}", sum_tree(tree));
}
