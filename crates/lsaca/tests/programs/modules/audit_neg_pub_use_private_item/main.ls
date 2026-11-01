// audit: pub use must not widen the visibility of a re-exported item.
mod helper;

pub use helper::only_in_crate;

fn main() {
    println!("{}", only_in_crate());
}
