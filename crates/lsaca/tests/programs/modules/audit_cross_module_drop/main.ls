// audit: a type with a Drop impl is defined in another module, constructed
// there and dropped in the root module.
mod guard;

fn main() {
    let guard = guard::make(1);
    println!("alive {}", guard.id);
}
