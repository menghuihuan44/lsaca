// A Drop value may not be moved out of a pattern binding (E0509).
struct Guard {
    name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let pair = (Guard { name: String::from("g") }, 7);
    let (guard, count) = pair;
    println!("{} {}", guard.name, count);
}
