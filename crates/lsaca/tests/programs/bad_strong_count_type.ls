// Rc::strong_count returns usize (v2.1).
fn main() {
    let value: Rc<i64> = Rc::new(1);
    let count: i64 = value.strong_count();
    println!("{}", count);
}
