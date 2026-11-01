// audit: a use alias binds the imported function under a new name.
mod math;
use math::add as plus;

fn main() {
    println!("{}", plus(2, 3));
}
