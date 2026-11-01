// audit: pub use can rename a re-exported item.
mod api;
use api::get_value;

fn main() {
    println!("{}", get_value());
}
