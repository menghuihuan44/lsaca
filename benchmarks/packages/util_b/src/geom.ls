// geom: small 2D geometry helpers for the util_b library package.

pub const MODULUS: i64 = 1000000007;

pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub fn new(x: i64, y: i64) -> Point {
        return Point { x: x, y: y };
    }

    pub fn translated(&self, dx: i64, dy: i64) -> Point {
        return Point { x: self.x + dx, y: self.y + dy };
    }

    pub fn manhattan(&self) -> i64 {
        return abs_value(self.x) + abs_value(self.y);
    }

    pub fn checksum(&self) -> i64 {
        return (self.x * 31 + self.y) % MODULUS;
    }
}

pub struct Rect {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}

impl Rect {
    pub fn new(left: i64, top: i64, right: i64, bottom: i64) -> Rect {
        return Rect { left: left, top: top, right: right, bottom: bottom };
    }

    pub fn width(&self) -> i64 {
        if self.right < self.left {
            return 0;
        }
        return self.right - self.left;
    }

    pub fn height(&self) -> i64 {
        if self.bottom < self.top {
            return 0;
        }
        return self.bottom - self.top;
    }

    pub fn area(&self) -> i64 {
        return self.width() * self.height();
    }

    pub fn contains(&self, point: &Point) -> bool {
        if point.x < self.left || point.x >= self.right {
            return false;
        }
        if point.y < self.top || point.y >= self.bottom {
            return false;
        }
        return true;
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        if self.right <= other.left || other.right <= self.left {
            return false;
        }
        if self.bottom <= other.top || other.bottom <= self.top {
            return false;
        }
        return true;
    }

    pub fn checksum(&self) -> i64 {
        let mut state = self.left * 31 + self.top * 17;
        state = (state + self.right * 13 + self.bottom * 7) % MODULUS;
        return state;
    }
}

pub fn abs_value(value: i64) -> i64 {
    if value < 0 {
        return -value;
    }
    return value;
}

pub fn distance_squared(a: &Point, b: &Point) -> i64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    return dx * dx + dy * dy;
}

pub fn distance_manhattan(a: &Point, b: &Point) -> i64 {
    return abs_value(a.x - b.x) + abs_value(a.y - b.y);
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;
    let origin = Point::new(0, 0);
    let target = Point::new(3, 4);
    acc = (acc * 31 + distance_squared(&origin, &target)) % MODULUS;
    acc = (acc * 31 + distance_manhattan(&origin, &target)) % MODULUS;
    let moved = target.translated(2, -1);
    acc = (acc * 31 + moved.checksum()) % MODULUS;
    acc = (acc * 31 + moved.manhattan()) % MODULUS;
    let frame = Rect::new(0, 0, 10, 8);
    acc = (acc * 31 + frame.width()) % MODULUS;
    acc = (acc * 31 + frame.height()) % MODULUS;
    acc = (acc * 31 + frame.area()) % MODULUS;
    acc = (acc * 31 + frame.checksum()) % MODULUS;
    if frame.contains(&moved) {
        acc = (acc * 31 + 1) % MODULUS;
    } else {
        acc = (acc * 31 + 2) % MODULUS;
    }
    let other = Rect::new(5, 2, 14, 12);
    if frame.overlaps(&other) {
        acc = (acc * 31 + 3) % MODULUS;
    } else {
        acc = (acc * 31 + 4) % MODULUS;
    }
    let tiny = Rect::new(20, 20, 22, 24);
    acc = (acc * 31 + tiny.area()) % MODULUS;
    return acc;
}
