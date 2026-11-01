// audit: inherent methods of a module type are callable from another module.
mod point;
use point::Point;

fn main() {
    let point = Point::new(3, 4);
    println!("{} {}", point.sum(), point.x);
}
