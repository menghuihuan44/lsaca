// Mutable references to heap types (&mut String, &mut Vec<T>, &mut Rc<T>)
// are legal in version 1.4: the callee mutates the caller's value in place.
fn append(text: &mut String) {
    text.push_str("!");
}

fn grow(values: &mut Vec<i64>) {
    values.push(4);
}

fn bump(cell: &mut Rc<i64>) {
    cell.set(cell.get() + 1);
}

fn main() {
    let mut text = String::from("hi");
    append(&mut text);
    println!("{}", text);

    let mut values = vec![1, 2, 3];
    grow(&mut values);
    println!("{:?}", values);

    let mut cell = Rc::new(5);
    bump(&mut cell);
    println!("{}", cell.get());
}
