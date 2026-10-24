struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let n: i64 = 1_000_000;
    let mut points: Vec<Point> = Vec::with_capacity(n as usize);
    for i in 0..n {
        points.push(Point { x: i, y: 2 * i });
    }
    let mut checksum: i64 = 0;
    for point in &points {
        checksum += point.x + point.y;
    }
    println!("checksum={}", checksum);
}
