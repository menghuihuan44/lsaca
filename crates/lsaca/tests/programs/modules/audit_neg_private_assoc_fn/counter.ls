pub struct Counter {
    pub value: i64,
}

impl Counter {
    pub fn new(value: i64) -> Counter {
        Counter { value }
    }

    fn hidden() -> i64 {
        7
    }
}
