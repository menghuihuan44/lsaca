pub struct Point {
    pub x: i64,
    pub y: i64,
}

pub fn origin() -> Point {
    Point { x: 1, y: 2 }
}

pub fn area(point: Point) -> i64 {
    point.x * point.y
}
