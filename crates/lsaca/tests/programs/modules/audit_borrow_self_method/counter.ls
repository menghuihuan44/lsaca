pub struct Counter {
    pub value: i64,
    label: String,
}

impl Counter {
    pub fn new(value: i64) -> Counter {
        Counter { value: value, label: String::from("count") }
    }

    pub fn bump(&mut self) {
        self.value = self.value + 1;
    }

    pub fn get(&self) -> i64 {
        self.value
    }
}

pub fn read(counter: &Counter) -> i64 {
    counter.get()
}
