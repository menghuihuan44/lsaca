// Types that implement Drop cannot be cloned in version 1.4 (E0277).
struct Guard {
    name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let guard = Guard { name: String::from("g") };
    let copy = guard.clone();
    println!("{}", copy.name);
}
