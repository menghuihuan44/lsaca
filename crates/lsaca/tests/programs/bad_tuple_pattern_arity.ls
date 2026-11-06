// A tuple struct pattern must bind exactly one value per field (v2.1).
struct Point(i64, i64);

fn main() {
    let point = Point(1, 2);
    match point {
        Point(x) => println!("{}", x),
    }
}
