// Vec::with_capacity preallocates storage; the vector starts empty.

fn main() {
    let mut numbers: Vec<i64> = Vec::with_capacity(8);
    println!("{} {}", numbers.len(), numbers.is_empty());
    numbers.push(1);
    numbers.push(2);
    numbers.push(3);
    println!("{:?}", numbers);

    let mut names: Vec<String> = Vec::with_capacity(2);
    names.push(String::from("hi"));
    names.push(String::from("there"));
    println!("{:?}", names);

    let mut points: Vec<Point> = Vec::with_capacity(2);
    points.push(Point { x: 1, y: 2 });
    println!("{}", points.len());
    points.push(Point { x: 3, y: 4 });
    println!("{} {}", points[0].x, points[1].y);
}

struct Point {
    x: i64,
    y: i64,
}
