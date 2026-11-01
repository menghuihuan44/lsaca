// audit: pub(crate) inherent methods, associated functions and free functions
// are visible across modules of the same crate.
mod tools;

fn main() {
    let tool = tools::Tool::new(4);
    println!("{} {}", tool.value(), tools::twice(3));
}
