trait Shape {
    fn area(&self) -> i64;
}

struct Unit {
    marker: i64,
}

impl Shape for Unit {
    fn area(&self) -> i64 {
        return self.marker;
    }
}

fn measure(shape: dyn Shape) -> i64 {
    return shape.area();
}

fn main() {
    println!("{}", measure(Unit { marker: 0 }));
}
