struct Node {
    value: i64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

fn leaf(value: i64) -> Node {
    Node { value: value, left: None, right: None }
}

fn node(value: i64, left: Node, right: Node) -> Node {
    Node { value: value, left: Some(Box::new(left)), right: Some(Box::new(right)) }
}

fn sum(tree: &Option<Box<Node>>) -> i64 {
    match tree {
        None => 0,
        Some(boxed) => {
            let current: &Node = boxed;
            current.value + sum(&current.left) + sum(&current.right)
        }
    }
}

fn rc_payload(cell: &Rc<i64>) -> i64 {
    let inner: &i64 = cell;
    *inner + 1
}

fn main() {
    let tree = Some(Box::new(node(1, node(2, leaf(4), leaf(5)), leaf(3))));
    println!("{}", sum(&tree));

    let shared = Rc::new(41);
    println!("{}", rc_payload(&shared));
    println!("{}", shared.strong_count());
}
