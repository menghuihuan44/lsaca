// audit: a uniform use path may descend through a child module.
mod api;
use api::inner::value;

fn main() {
    println!("{}", value());
}
