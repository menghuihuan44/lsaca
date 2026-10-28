fn main() {
    let mut value = 1;
    let reader = &value;
    value = 2;
    println!("{}", *reader);
}
