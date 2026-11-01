// audit: enum tuple-variant payloads are public whenever the enum is visible.
mod shape;
use shape::Shape;

fn main() {
    let shape = Shape::Rect(3);
    let area = match shape {
        Shape::Circle(radius) => radius,
        Shape::Rect(side) => side * side,
        Shape::Empty => 0,
    };
    println!("{}", area);
}
