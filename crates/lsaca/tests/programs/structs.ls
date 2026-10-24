struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn new(x: i64, y: i64) -> Point {
        Point { x: x, y: y }
    }

    fn sum(&self) -> i64 {
        self.x + self.y
    }

    fn scale(&mut self, factor: i64) {
        self.x = self.x * factor;
        self.y = self.y * factor;
    }
}

fn main() {
    let mut p = Point::new(2, 3);
    println!("{}", p.sum());
    p.scale(10);
    println!("{} {} {}", p.x, p.y, p.sum());
}
