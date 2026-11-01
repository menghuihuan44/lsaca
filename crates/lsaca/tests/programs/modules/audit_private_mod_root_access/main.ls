// audit: an item with private visibility declared at the crate root is still
// visible from a descendant module.
mod hidden;
mod caller;

fn main() {
    println!("{}", caller::value());
}
