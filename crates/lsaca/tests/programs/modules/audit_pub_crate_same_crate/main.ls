// audit: pub(crate) items are visible anywhere inside the same crate.
mod util;
use util::helper;

fn main() {
    println!("{}", helper() + util::LIMIT);
}
