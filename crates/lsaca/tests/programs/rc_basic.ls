// Shared, mutable heap cells: Rc<T>.
fn main() {
    let a = Rc::new(10);
    let b = a.clone();
    println!("{} {}", a.get(), b.get());
    println!("strong_count = {}", a.strong_count());

    // The cell is shared, so it can be updated through any handle without a
    // mutable binding.
    b.set(99);
    println!("{} {}", a.get(), b.get());

    let from_function = make_counter(7);
    println!("{}", from_function.get());
}

fn make_counter(start: i64) -> Rc<i64> {
    Rc::new(start)
}
