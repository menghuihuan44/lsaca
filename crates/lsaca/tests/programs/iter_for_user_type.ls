struct Counter {
    value: i64,
}

impl Iterator<i64> for Counter {
    fn next(&mut self) -> Option<i64> {
        if self.value >= 3 {
            return None;
        }
        self.value = self.value + 1;
        return Some(self.value);
    }
}

fn main() {
    let first = Counter { value: 0 };
    for value in first {
        println!("{}", value);
    }

    let second = Counter { value: 0 };
    for value in second.map(|n| n * 2) {
        println!("{}", value);
    }
}
