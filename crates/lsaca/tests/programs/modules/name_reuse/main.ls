mod one;
mod two;

fn main() {
    println!("{}", one::value() * two::value());
}
