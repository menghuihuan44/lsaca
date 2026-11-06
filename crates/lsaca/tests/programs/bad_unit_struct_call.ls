// A unit struct is a value, not a callable constructor (v2.1).
struct Marker;

fn main() {
    let marker = Marker();
    println!("{:?}", marker);
}
