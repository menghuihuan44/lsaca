// audit: pub use re-exports a type from a private submodule.
mod api;
use api::Counter;

fn main() {
    let counter = Counter::new(7);
    println!("{}", counter.value());
}
