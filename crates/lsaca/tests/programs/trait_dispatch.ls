// Inherent methods win over trait implementations with the same name.
trait Sound {
    fn speak(&self) -> String;
}

struct Dog {
    name: String,
}

impl Dog {
    fn speak(&self) -> String {
        return self.name + " says woof (inherent)";
    }
}

impl Sound for Dog {
    fn speak(&self) -> String {
        return self.name + " says woof (trait)";
    }
}

trait Count {
    fn count(&self) -> i64;
}

struct Pair {
    left: i64,
    right: i64,
}

impl Count for Pair {
    fn count(&self) -> i64 {
        return self.left + self.right;
    }
}

fn main() {
    let dog = Dog { name: String::from("rex") };
    println!("{}", dog.speak());
    let pair = Pair { left: 1, right: 2 };
    println!("{}", pair.count());
}
