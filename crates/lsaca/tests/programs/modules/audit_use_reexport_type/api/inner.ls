pub struct Counter {
    value: i64,
}

impl Counter {
    pub fn new(value: i64) -> Counter {
        Counter { value: value }
    }

    pub fn value(&self) -> i64 {
        self.value
    }
}
