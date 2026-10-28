// A for loop over a Vec moves the vector; non-Copy element bindings own the
// element and release it at the end of each iteration.
struct Item {
    name: String,
}

impl Drop for Item {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let items = vec![
        Item { name: String::from("a") },
        Item { name: String::from("b") },
    ];
    for item in items {
        println!("item {}", item.name);
    }
    println!("done");
}
