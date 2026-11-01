// audit: pub(crate) fields are visible across modules of the same crate.
mod holder;

fn main() {
    let mut counter = holder::make();
    counter.count = counter.count + 1;
    println!("{}", counter.count);
}
