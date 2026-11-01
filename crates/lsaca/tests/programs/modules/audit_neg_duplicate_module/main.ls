// audit: declaring the same module twice in one module is rejected (E0428).
mod dup;
mod dup;

fn main() {
}
