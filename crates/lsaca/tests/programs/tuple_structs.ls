// Tuple structs, newtypes, unit structs and generic tuple structs (v2.1).
struct Point(i64, i64);

struct Meters(i64);

struct Unit;

struct Pair<T>(T, T);

impl Point {
    fn new(x: i64, y: i64) -> Point {
        Point(x, y)
    }

    fn sum(&self) -> i64 {
        self.0 + self.1
    }

    fn set_x(&mut self, x: i64) {
        self.0 = x;
    }
}

impl Meters {
    fn value(&self) -> i64 {
        self.0
    }
}

fn main() {
    let mut p = Point::new(3, 4);
    println!("{} {} {}", p.0, p.1, p.sum());
    p.set_x(10);
    println!("{}", p.sum());

    let m = Meters(7);
    println!("{}", m.value());

    let first = Unit;
    let second = Unit;
    let _ = first;
    let _ = second;
    println!("unit ok");

    let pair = Pair(1, 2);
    println!("{} {}", pair.0, pair.1);

    let total = match p {
        Point(x, y) => x + y,
    };
    println!("{}", total);

    println!("{:?}", p);
    println!("{:?}", m);
    println!("{:?}", first);
}