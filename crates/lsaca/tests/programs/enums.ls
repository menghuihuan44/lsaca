enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Unit,
}

fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Circle(r) => 3.14 * r * r,
        Shape::Rectangle(w, h) => w * h,
        Shape::Unit => 0.0,
    }
}

fn describe(value: Option<i64>) -> String {
    match value {
        Some(n) => format!("some {}", n),
        None => String::from("none"),
    }
}

fn main() {
    println!("{}", area(Shape::Circle(2.0)));
    println!("{}", area(Shape::Rectangle(3.0, 4.0)));
    println!("{}", area(Shape::Unit));
    println!("{} {}", describe(Some(7)), describe(None));
}
