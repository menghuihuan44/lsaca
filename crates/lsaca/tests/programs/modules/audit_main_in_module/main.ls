// audit: fn main declared inside a module is an ordinary function; only the
// root crate's main is the entry point.
mod app;

fn main() {
    println!("{}", app::main());
}
