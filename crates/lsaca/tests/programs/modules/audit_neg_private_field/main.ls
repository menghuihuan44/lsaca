// audit: reading a private struct field from another module is rejected (E0616).
mod counter;

fn main() {
    let counter = counter::make(1);
    println!("{}", counter.value);
}
