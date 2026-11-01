// audit: a module imports an item of its own child module with self.
mod outer;

fn main() {
    println!("{}", outer::get());
}
