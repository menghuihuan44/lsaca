// audit: a use group imports several names from one path.
mod math;
use crate::math::{add, sub};

fn main() {
    println!("{} {}", add(5, 2), sub(5, 2));
}
