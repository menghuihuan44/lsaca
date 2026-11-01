// audit: a private item is visible in the defining module and its descendants.
mod parent;

fn main() {
    println!("{}", parent::call());
}
