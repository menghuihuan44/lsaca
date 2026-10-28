struct Point {
    x: i64,
    y: i64,
}

enum Shape {
    Circle(i64),
    Unit,
}

fn describe(point: &Point) -> i64 {
    match point {
        Point { x, y } => *x + *y,
    }
}

fn area(shape: &Shape) -> i64 {
    match shape {
        Shape::Circle(radius) => *radius * *radius * 3,
        Shape::Unit => 0,
    }
}

fn main() {
    let point = Point { x: 3, y: 4 };
    println!("{}", describe(&point));

    let shape = Shape::Circle(2);
    println!("{}", area(&shape));

    let maybe = Some(7);
    match &maybe {
        Some(value) => println!("{}", *value),
        None => println!("none"),
    }

    let mut counter = 0;
    let counter_ref = &mut counter;
    match counter_ref {
        value => {
            *value = 9;
        }
    }
    println!("{}", counter);
}
