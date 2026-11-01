pub struct Counter {
    value: i64,
}

pub fn make(value: i64) -> Counter {
    Counter { value: value }
}
