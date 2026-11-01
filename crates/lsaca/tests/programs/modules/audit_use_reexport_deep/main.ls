// audit: a re-export can name a leaf several modules deep.
mod api;
use api::helper;

fn main() {
    println!("{}", helper());
}
