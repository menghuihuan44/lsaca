// audit: pub use can re-export a whole module under an alias.
mod api;

fn main() {
    println!("{}", api::inner_api::value());
}
