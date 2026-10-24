enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Empty,
}

fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Circle(radius) => 3.14 * radius * radius,
        Shape::Rectangle(width, height) => width * height,
        Shape::Empty => 0.0,
    }
}

fn main() {
    let shapes = vec![Shape::Circle(1.0), Shape::Rectangle(2.0, 3.0), Shape::Empty];
    for shape in shapes {
        println!("area = {}", area(shape));
    }
}
