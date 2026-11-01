// audit: calling a private function of another module is rejected (E0603).
mod helper;

fn main() {
    println!("{}", helper::hidden());
}
