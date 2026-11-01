// audit: aliases are allowed inside a use group.
mod math;
use crate::math::{add as plus, sub as minus};

fn main() {
    println!("{} {}", plus(5, 2), minus(5, 2));
}
