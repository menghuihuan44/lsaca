// A tuple struct constructor needs exactly one argument per field (v2.1).
struct Point(i64, i64);

fn main() {
    let point = Point(1);
    println!("{}", point.0);
}
