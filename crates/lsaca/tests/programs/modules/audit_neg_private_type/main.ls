// audit: naming a private type of another module is rejected (E0603).
mod helper;

fn consume(hidden: helper::Hidden) -> i64 {
    hidden.value
}

fn main() {
}
