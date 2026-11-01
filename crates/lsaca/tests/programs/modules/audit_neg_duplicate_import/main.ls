// audit: importing the same name twice in one module is rejected (E0252).
mod math;
use math::add;
use math::add;

fn main() {
    println!("{}", add(1, 2));
}
