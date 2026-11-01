// audit: moving a non-Copy value of a module-defined type while it is borrowed
// reports E0505 in the root module.
mod item;

fn main() {
    let value = item::build(5);
    let reference = &value;
    let moved = value;
    println!("{} {}", (*reference).value, moved.value);
}
