// binary_trees_refs: identical tree and checksum to binary_trees; the lsaca
// version differs only by traversing through borrowed references.
// binary_trees: build a perfect binary tree (leaves carry consecutive
// payloads), recursively sum the payloads. Rust nodes are Box-allocated and
// traversed by moving out of the boxes (no payload copies).
// Optional argument: depth (default 16, the medium scale).
struct Node {
    value: i64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let depth = arg_i64(&args, 0, 16);
    let tree = build(depth, 0);
    println!("checksum={}", sum_tree(tree));
}
