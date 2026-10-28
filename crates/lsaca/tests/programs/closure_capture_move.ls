// Closures capture by value: Copy captures are copied, non-Copy captures are
// moved into the closure environment and released with it.
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let text = String::from("lsaca");
    let loud = Loud { name: String::from("env") };
    let show = || println!("captured {} {}", text, loud.name);
    show();
    show();

    let items = vec![1, 2];
    let counted = || items.len() + 1;
    println!("count {}", counted());
    println!("done");
}
