// audit: glob imports are rejected (E0432).
mod helper;
use helper::*;

fn main() {
    println!("{}", value());
}
