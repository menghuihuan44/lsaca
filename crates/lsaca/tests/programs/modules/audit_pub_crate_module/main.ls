// audit: a pub(crate) module is reachable from any module in the crate.
mod outer;

fn main() {
    println!("{}", outer::inner::value());
}
