// audit: calling a private inherent method from another module is rejected
// (E0624).
mod counter;

fn main() {
    let counter = counter::Counter::new();
    println!("{}", counter.secret());
}
