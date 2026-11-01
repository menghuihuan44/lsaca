// audit: a private associated function is E0624 (Rust parity).
mod counter;

use counter::Counter;

fn main() {
    let counter = Counter::new(3);
    println!("{} {}", counter.value, Counter::hidden());
}
