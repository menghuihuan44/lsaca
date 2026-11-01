// audit: a child module imports from its parent with super and self paths.
mod outer;

fn main() {
    println!("{}", outer::inner::value());
}
