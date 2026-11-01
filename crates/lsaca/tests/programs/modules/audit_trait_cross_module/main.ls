// audit: a trait from another module can be imported, implemented locally and
// called on a value of the importing module's struct.
mod behavior;
use behavior::Speak;

struct Local {
    value: i64,
}

impl Speak for Local {
    fn speak(&self) -> i64 {
        self.value * 2
    }
}

fn main() {
    let dog = behavior::Dog { value: 3 };
    let local = Local { value: 4 };
    println!("{} {}", dog.speak(), local.speak());
}
