// audit: a module file that includes itself through a mod declaration is a
// circular module graph and is rejected (E0001).
mod main;

fn main() {
}
