// Destructuring a value whose own type implements Drop and moving a field out is E0509.
struct Guard {
    name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

struct Pair {
    guard: Guard,
    count: i64,
}

impl Drop for Pair {
    fn drop(&mut self) {
        println!("drop pair");
    }
}

fn main() {
    let pair = Pair { guard: Guard { name: String::from("g") }, count: 7 };
    let Pair { guard, count } = pair;
    println!("{} {}", guard.name, count);
}
