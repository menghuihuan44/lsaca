// audit: two imports binding the same alias in one module are rejected
// (E0252).
mod first;
mod second;
use first::value as f;
use second::value as f;

fn main() {
    println!("{}", f());
}
