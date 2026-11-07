// Loops have type (); break with a value is not supported (E0308).
fn main() {
    loop {
        break 1;
    }
}
