fn main() {
    let value: i64 = 7;
    let pointer: *const i64 = &value as *const i64;
    let read: i64 = *pointer;
    println!("{}", read);
}
