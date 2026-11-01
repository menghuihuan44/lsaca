// audit: naming a private enum of another module is rejected (E0603).
mod helper;

fn main() {
    let value = helper::Hidden::First;
}
