// Box<T> is an owned heap indirection with value semantics: cloning a box
// deep-copies the payload, so a box can be used to build recursive types.
struct Node {
    value: i64,
    next: Option<Box<Node>>,
}

fn make(value: i64) -> Box<i64> {
    Box::new(value)
}

fn main() {
    let a = Box::new(10);
    println!("{}", a.get());

    // Cloning a box deep-copies its payload: mutating the clone leaves the
    // original untouched.
    let mut b = a.clone();
    b.set(20);
    println!("{} {}", b.get(), a.get());
    println!("{}", b == Box::new(20));

    let head = Node {
        value: 1,
        next: Some(Box::new(Node { value: 2, next: None })),
    };
    let second = head.next.clone().unwrap().get();
    println!("{}", second.value);
    println!("{}", make(7).get());

    // Displaying a box displays the payload, like Rust.
    let mut text = Box::new(String::from("hi"));
    println!("{} [{:?}]", text, text);
    text.set(String::from("bye"));
    println!("{}", text.get());

    let mut messages = Vec::new();
    messages.push(Box::new(String::from("first")));
    messages.push(Box::new(String::from("second")));
    println!("{}", messages.len());
    println!("{}", messages[1].get());
}
