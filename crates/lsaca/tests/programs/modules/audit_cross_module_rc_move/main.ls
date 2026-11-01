// audit: an ORC handle created in another module is cloned and moved across
// the module boundary; reference counting stays consistent.
mod refs;

fn main() {
    let first = refs::shared();
    let second = first.clone();
    let count = first.strong_count();
    println!("{} {} {}", first.get(), second.get(), count);
}
