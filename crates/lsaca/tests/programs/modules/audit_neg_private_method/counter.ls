pub struct Counter {
    value: i64,
}

impl Counter {
    pub fn new() -> Counter {
        Counter { value: 1 }
    }

    fn secret(&self) -> i64 {
        self.value
    }
}
