// audit: a private child module cannot be reached from outside its parent
// module (E0603).
mod outer;

fn main() {
    println!("{}", outer::inner::value());
}
