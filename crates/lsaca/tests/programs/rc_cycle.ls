// A reference cycle created on purpose: the ORC cycle collector must reclaim
// it at shutdown. Debug builds would report a leak if it did not.
struct Node {
    value: i64,
    next: Option<Rc<Node>>,
}

fn main() {
    let a = Rc::new(Node { value: 1, next: None });
    let b = Rc::new(Node { value: 2, next: None });

    let a2 = a;
    let b2 = b;

    // a -> b -> a
    b.set(Node { value: 2, next: Some(a2) });
    a.set(Node { value: 1, next: Some(b2) });

    println!("{}", a.get().value);
    if let Some(next) = a.get().next {
        println!("{}", next.get().value);
    }
    println!("done");
}
