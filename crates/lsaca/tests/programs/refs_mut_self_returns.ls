struct Counter {
    count: i64,
}

impl Counter {
    fn count_mut(&mut self) -> &mut i64 {
        &mut self.count
    }

    fn seen(&mut self) -> &i64 {
        &self.count
    }
}

fn main() {
    let mut counter = Counter { count: 0 };
    *counter.count_mut() += 1;
    println!("{}", counter.count);

    let seen = counter.seen();
    println!("{}", *seen);
}
