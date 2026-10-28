fn main() {
    let mut value = 1;
    let writer = &mut value;
    let reader = &value;
    *writer = 2;
    println!("{}", *reader);
}
