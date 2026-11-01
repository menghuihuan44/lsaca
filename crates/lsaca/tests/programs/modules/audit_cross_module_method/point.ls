pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub fn new(x: i64, y: i64) -> Point {
        Point { x: x, y: y }
    }

    pub fn sum(&self) -> i64 {
        self.x + self.y
    }
}
