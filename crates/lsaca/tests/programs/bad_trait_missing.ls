// Implementing a trait without all of its methods must be rejected (E0046).
trait Shape {
    fn area(&self) -> i64;
    fn name(&self) -> String;
}

struct Square {
    side: i64,
}

impl Shape for Square {
    fn area(&self) -> i64 {
        return self.side * self.side;
    }
}

fn main() {
    let square = Square { side: 2 };
    println!("{}", square.area());
}
