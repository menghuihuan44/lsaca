// Mutating a tuple field requires a mut binding (v2.1).
struct Point(i64, i64);

fn main() {
    let point = Point(1, 2);
    point.0 = 3;
    println!("{}", point.0);
}
