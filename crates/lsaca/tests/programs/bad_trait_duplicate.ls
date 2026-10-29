// Two implementations of the same trait for the same type (E0119).
trait Shape {
    fn area(&self) -> i64;
}

struct Square {
    side: i64,
}

impl Shape for Square {
    fn area(&self) -> i64 {
        return self.side * self.side;
    }
}

impl Shape for Square {
    fn area(&self) -> i64 {
        return 0;
    }
}

fn main() {
    let square = Square { side: 2 };
    println!("{}", square.area());
}
