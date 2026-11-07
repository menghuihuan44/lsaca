// Instantiating a user generic struct with a container of references is rejected.
struct Holder<T> {
    value: T,
}

fn main() {
    let number = 1;
    let holder: Holder<Vec<&i64>> = Holder { value: vec![&number] };
    println!("{}", holder.value.len());
}
