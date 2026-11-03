trait Greeter {
    fn name(&self) -> i64;
    fn describe(&self) -> i64 {
        return self.name() + 100;
    }
}

struct Base { id: i64 }
struct Special { id: i64 }

impl Greeter for Base {
    fn name(&self) -> i64 { return self.id; }
}

impl Greeter for Special {
    fn name(&self) -> i64 { return self.id; }
    fn describe(&self) -> i64 { return 900; }
}

fn main() {
    let base = Base { id: 1 };
    let special = Special { id: 2 };
    println!("{} {}", base.describe(), special.describe());
}
