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

    fn bump(&mut self) {
        self.x = self.x + 1;
    }
}

fn double(value: &i64) -> i64 {
    *value * 2
}

fn first(values: &Vec<i64>) -> &i64 {
    &values[0]
}

fn describe(point: &Point) -> String {
    format!("({}, {})", point.x, point.y)
}

fn main() {
    let n = 21;
    println!("{}", double(&n));

    let values = vec![3, 4, 5];
    let head = first(&values);
    println!("{}", *head);

    let mut point = Point::new(1, 2);
    println!("{}", describe(&point));
    point.bump();
    println!("{}", point.sum());
}
