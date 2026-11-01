// audit: an owned value returned by another module is moved across the
// module boundary and outlives the binding it was moved into.
mod lists;

fn main() {
    let words = lists::words();
    let moved = words;
    println!("{}", moved.len());
}
