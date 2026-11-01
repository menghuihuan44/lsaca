// audit: a use path may start with an item of the current module.
mod math;
use math::add;

fn main() {
    println!("{}", add(2, 3));
}
