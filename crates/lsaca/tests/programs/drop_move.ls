// Moving a value transfers ownership: its Drop hook runs exactly once, at
// the end of the scope that owns it.
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn take(value: Loud) {
    println!("took {}", value.name);
}

fn main() {
    let first = Loud { name: String::from("first") };
    take(first);
    println!("after take");
    let second = Loud { name: String::from("second") };
    let moved = second;
    println!("moved to {}", moved.name);
    println!("end");
}
