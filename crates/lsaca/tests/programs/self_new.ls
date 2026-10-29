// Self in expression position: Self::new(..) and Self { .. } inside impls.
struct Counter {
    value: i64,
}

impl Counter {
    fn new(value: i64) -> Self {
        return Self { value: value };
    }

    fn bump(&mut self) -> Self {
        self.value = self.value + 1;
        return Self::new(self.value);
    }
}

fn main() {
    let mut counter = Counter::new(1);
    let mut next = counter.bump();
    println!("{} {}", counter.value, next.value);
    let again = next.bump();
    println!("{}", again.value);
}
