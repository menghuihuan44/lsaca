// audit: assigning through a value while it is shared-borrowed inside a module
// reports E0506 and the diagnostic names the module file.
mod helper;

fn main() {
    println!("{}", helper::write_while_shared());
}
