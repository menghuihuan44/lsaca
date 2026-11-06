// Tuple field indices are checked against the field count (v2.1).
struct Point(i64, i64);

fn main() {
    let point = Point(1, 2);
    println!("{}", point.2);
}
