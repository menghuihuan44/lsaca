// Cloning Rc retains the same cell; cloning Box deep-copies its payload.
struct Wrapped {
    label: String,
    boxed: Box<i64>,
}

fn main() {
    let cell = Rc::new(10);
    let handle = cell.clone();
    println!("{}", cell == handle);
    handle.set(20);
    println!("{} {}", cell.get(), handle.get());
    println!("strong_count = {}", cell.strong_count());

    let mut boxed = Box::new(String::from("hi"));
    let boxed_copy = boxed.clone();
    boxed.set(String::from("changed"));
    println!("{} {}", boxed.get(), boxed_copy.get());

    let wrapped = Wrapped {
        label: String::from("w"),
        boxed: Box::new(5),
    };
    let wrapped_copy = wrapped.clone();
    println!("{} {} {}", wrapped.label, wrapped_copy.label, wrapped_copy.boxed.get());
}
