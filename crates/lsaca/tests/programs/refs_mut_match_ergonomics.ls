struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let mut point = Point { x: 1, y: 2 };
    match &mut point {
        Point { x, y } => {
            *x = 10;
            *y = 20;
        }
    }
    println!("{} {}", point.x, point.y);

    let mut counter = 3;
    let counter_ref = &mut counter;
    match counter_ref {
        value => *value = 7,
    }
    println!("{}", counter);

    let mut maybe: Option<i64> = Some(1);
    while let Some(value) = &mut maybe {
        *value = *value + 1;
        break;
    }
    println!("{:?}", maybe);

    let mut other: Option<i64> = Some(1);
    if let Some(value) = &mut other {
        *value = 9;
    }
    println!("{:?}", other);
}
