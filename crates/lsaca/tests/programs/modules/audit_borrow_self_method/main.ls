// audit: a shared borrow created in the root module is used through a
// function of another module, then ends at its last use so a later mutable
// method call on the same value is accepted across the module boundary.
mod counter;
use counter::Counter;

fn main() {
    let mut counter = Counter::new(1);
    let shared = &counter;
    let seen = counter::read(shared);
    counter.bump();
    println!("{} {}", seen, counter.get());
}
