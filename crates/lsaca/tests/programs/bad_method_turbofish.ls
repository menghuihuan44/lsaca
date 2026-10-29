// Generic methods do not exist in this language version. A turbofish on a
// non-generic method must be rejected instead of being silently ignored.

struct Point {
    x: i64,
}

impl Point {
    fn get(&self) -> i64 {
        return self.x;
    }
}

fn main() {
    let point = Point { x: 1 };
    println!("{}", point.get::<i64>());
}
