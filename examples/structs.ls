struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Point {
        Point { x: x, y: y }
    }

    fn manhattan(&self) -> f64 {
        abs(self.x) + abs(self.y)
    }

    fn scale(&mut self, factor: f64) {
        self.x = self.x * factor;
        self.y = self.y * factor;
    }
}

fn main() {
    let mut p = Point::new(3.0, -4.0);
    println!("manhattan distance: {}", p.manhattan());
    p.scale(2.0);
    println!("after scaling: {} {}", p.x, p.y);
}
