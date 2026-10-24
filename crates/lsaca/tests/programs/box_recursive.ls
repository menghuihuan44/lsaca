// Recursive data structures use Box<T> for the indirection. Walking the
// chain copies the boxes, so the collector can prove that no cycle exists.
struct Node {
    value: i64,
    next: Option<Box<Node>>,
}

fn sum(node: Option<Box<Node>>) -> i64 {
    match node {
        Some(boxed) => {
            let inner = boxed.get();
            inner.value + sum(inner.next)
        }
        None => 0,
    }
}

fn depth(node: Option<Box<Node>>) -> i64 {
    match node {
        Some(boxed) => 1 + depth(boxed.get().next),
        None => 0,
    }
}

fn main() {
    let mut head = Box::new(Node { value: 5, next: None });
    let mut index = 0;
    while index < 4 {
        head = Box::new(Node {
            value: index,
            next: Some(head),
        });
        index = index + 1;
    }
    println!("{}", sum(Some(head)));
    println!("{}", depth(Some(head)));
}
