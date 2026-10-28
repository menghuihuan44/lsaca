// Rc handles compare by identity: two handles are equal exactly when they
// refer to the same cell. Two distinct cells that hold equal payloads are not
// equal, and comparing the handles of a cycle terminates.
fn main() {
    let a = Rc::new(10);
    let b = a.clone();
    let c = Rc::new(10);
    println!("{}", a == b);
    println!("{}", a == c);
    println!("{}", a != c);
    println!("{}", a.get() == c.get());
}
