// audit: an explicit crate:: path imports an item of another module.
mod math;
use crate::math::add;

fn main() {
    println!("{}", add(5, 5));
}
