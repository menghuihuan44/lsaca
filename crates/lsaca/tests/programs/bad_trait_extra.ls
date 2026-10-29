// Implementing a method the trait does not declare must be rejected (E0407).
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

    fn perimeter(&self) -> i64 {
        return self.side * 4;
    }
}

fn main() {
    let square = Square { side: 2 };
    println!("{}", square.area());
}
