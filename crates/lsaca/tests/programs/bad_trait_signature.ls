// The implementation signature must match the trait declaration (E0053).
trait Scale {
    fn scale(&self, factor: i64) -> i64;
}

struct Value {
    inner: i64,
}

impl Scale for Value {
    fn scale(&self, factor: bool) -> i64 {
        return self.inner;
    }
}

fn main() {
    let value = Value { inner: 1 };
    println!("{}", value.scale(2));
}
