// audit: the same item name may be defined in different modules.
mod a;
mod b;

fn main() {
    println!("{}", a::value() + b::value());
}
