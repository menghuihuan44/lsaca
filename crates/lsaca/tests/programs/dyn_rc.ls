trait Greet {
    fn hello(&self) -> String;
    fn loud(&self) -> String;
}

struct Cat {
    name: String,
}

impl Greet for Cat {
    fn hello(&self) -> String {
        return format!("meow {}", self.name);
    }

    fn loud(&self) -> String {
        return self.hello().to_uppercase();
    }
}

fn main() {
    let cat = Cat { name: String::from("tom") };
    let first: Rc<dyn Greet> = Rc::new(cat);
    let second = first.clone();
    println!("{}", first.hello());
    println!("{}", second.loud());
    println!("{}", first.strong_count());
}
