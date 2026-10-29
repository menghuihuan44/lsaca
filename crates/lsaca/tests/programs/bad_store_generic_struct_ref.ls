// Instantiating a user generic struct with a reference is E0498.
struct Holder<T> {
    value: T,
}

fn main() {
    let number = 1;
    let reference = &number;
    let holder: Holder<&i64> = Holder { value: reference };
    println!("{}", *holder.value);
}
