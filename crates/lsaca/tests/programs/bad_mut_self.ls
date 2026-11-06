// A self receiver cannot be declared mut (v2.1).
struct Counter {
    count: i64,
}

impl Counter {
    fn bump(mut self) -> i64 {
        self.count += 1;
        self.count
    }
}

fn main() {
    let counter = Counter { count: 0 };
    println!("{}", counter.bump());
}
