// audit: pub use must not widen the visibility of a re-exported module.
mod helper;

pub use helper::inner;

fn main() {
    println!("{}", inner::value());
}
