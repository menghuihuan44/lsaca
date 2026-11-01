// audit: using a private constant of another module is rejected (E0603).
mod helper;

fn main() {
    println!("{}", helper::LIMIT);
}
