// audit: an import colliding with a local item is rejected (E0255).
mod math;
use math::add;

fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    println!("{}", add(1, 2));
}
