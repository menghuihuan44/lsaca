// A trait cannot be used as a type: no trait objects in v1.5 (E0412).
trait Shape {
    fn area(&self) -> i64;
}

fn measure(shape: Shape) -> i64 {
    return shape.area();
}

fn main() {
    println!("{}", 1);
}
