// User-defined traits with type parameters, implemented for user types.
trait Describe<T> {
    fn describe(&self, extra: T) -> String;
}

struct Point {
    x: i64,
    y: i64,
}

impl Describe<i64> for Point {
    fn describe(&self, extra: i64) -> String {
        return format!("point({}, {}) + {}", self.x, self.y, extra);
    }
}

struct Tag {
    label: String,
}

impl Describe<String> for Tag {
    fn describe(&self, extra: String) -> String {
        return self.label + ":" + extra;
    }
}

trait Sizer {
    fn size(&self) -> i64;
}

impl Sizer for Point {
    fn size(&self) -> i64 {
        return 2;
    }
}

impl Sizer for Tag {
    fn size(&self) -> i64 {
        return self.label.len();
    }
}

fn main() {
    let point = Point { x: 1, y: 2 };
    println!("{}", point.describe(3));
    println!("{}", point.size());

    let tag = Tag { label: String::from("t") };
    println!("{}", tag.describe(String::from("v")));
    println!("{}", tag.size());
}
