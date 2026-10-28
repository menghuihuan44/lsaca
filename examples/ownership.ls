struct Resource {
    name: String,
}

impl Resource {
    fn new(name: String) -> Resource {
        Resource { name: name }
    }
}

impl Drop for Resource {
    fn drop(&mut self) {
        println!("releasing {}", self.name);
    }
}

fn consume(resource: Resource) {
    println!("consuming {}", resource.name);
}

fn rename(text: &mut String) {
    text.push_str("!");
}

fn main() {
    // Copy values are copied, so both bindings stay usable.
    let count = 7;
    let copy = count;
    println!("copy = {} count = {}", copy, count);

    // Move-only values are moved unless cloned.
    let original = String::from("lsaca");
    let clone = original.clone();
    println!("original = {} clone = {}", original, clone);

    // Passing an owned value to a by-value parameter moves it.
    let first = Resource::new(String::from("first"));
    consume(first);

    // The drop hook runs when the last owner is released.
    let second = Resource::new(String::from("second"));
    println!("second still alive: {}", second.name);

    // A mutable reference borrows a heap value without moving it.
    let mut text = String::from("hi");
    rename(&mut text);
    println!("text = {}", text);
}
