// An empty array literal needs an element type hint (E0282).
fn main() {
    let values = [];
    println!("{}", values.len());
}
