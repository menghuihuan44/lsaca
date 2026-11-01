// audit: a shared borrow of a module value ends at its last use, then the
// (non-Copy) value may be moved across the module boundary.
mod item;
use item::Item;

fn main() {
    let value = item::build(5);
    let seen = item::inspect(&value);
    let moved = value;
    println!("{} {}", seen, moved.value);
}
