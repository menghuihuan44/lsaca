// Loops other than `loop` cannot produce a value: break with a value in a
// while or for loop is E0571.
fn main() {
    while true {
        break 1;
    }
}
