trait Shape {
    fn name(&self) -> String;
    fn area(&self) -> i64;
}

struct Rect {
    width: i64,
    height: i64,
}

impl Shape for Rect {
    fn name(&self) -> String {
        return String::from("rect");
    }

    fn area(&self) -> i64 {
        return self.width * self.height;
    }
}

struct Square {
    side: i64,
}

impl Shape for Square {
    fn name(&self) -> String {
        return String::from("square");
    }

    fn area(&self) -> i64 {
        return self.side * self.side;
    }
}

fn describe(shape: &dyn Shape) -> String {
    return format!("{}={}", shape.name(), shape.area());
}

fn stacked_total(shapes: &Vec<Box<dyn Shape>>) -> i64 {
    let mut total = 0;
    let mut index = 0;
    while index < shapes.len() {
        total = total + shapes[index].area();
        index = index + 1;
    }
    return total;
}

fn main() {
    let rect = Rect { width: 3, height: 4 };
    let square = Square { side: 5 };
    println!("{}", describe(&rect));
    println!("{}", describe(&square));

    let mut shapes: Vec<Box<dyn Shape>> = Vec::new();
    shapes.push(Box::new(Rect { width: 2, height: 5 }));
    shapes.push(Box::new(Square { side: 3 }));
    println!("{}", stacked_total(&shapes));

    let mut boxed: Box<dyn Shape> = Box::new(Square { side: 7 });
    println!("{}", boxed.area());
    boxed = Box::new(Rect { width: 2, height: 6 });
    println!("{}", boxed.area());
}
