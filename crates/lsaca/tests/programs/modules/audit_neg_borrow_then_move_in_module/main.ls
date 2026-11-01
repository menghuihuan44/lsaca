// audit: move-while-borrowed inside a module reports E0505 and the diagnostic
// names the module file.
mod helper;

fn main() {
    println!("{}", helper::moved_while_borrowed());
}
