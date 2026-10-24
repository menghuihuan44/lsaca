struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let n: i64 = 1000000;
    // Pre-allocate the backing storage exactly like the C and Rust versions do.
    let mut points: Vec<Point> = Vec::with_capacity(n);
    let mut i: i64 = 0;
    while i < n {
        points.push(Point { x: i, y: 2 * i });
        i += 1;
    }
    let mut checksum: i64 = 0;
    for p in points {
        checksum += p.x + p.y;
    }
    println!("checksum={}", checksum);
}
