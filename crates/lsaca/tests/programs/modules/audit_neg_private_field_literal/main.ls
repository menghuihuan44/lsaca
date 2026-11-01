// audit: naming a private struct field in a struct literal from another module
// is rejected (E0451).
mod counter;

fn main() {
    let counter = counter::Counter { value: 1 };
    println!("{}", counter.value);
}
