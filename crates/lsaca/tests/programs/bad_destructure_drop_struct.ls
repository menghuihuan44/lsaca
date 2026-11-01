struct Holder {
    name: String,
}

impl Drop for Holder {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

enum Wrapper {
    Text(String),
    Number(i64),
}

impl Drop for Wrapper {
    fn drop(&mut self) {
        println!("drop wrapper");
    }
}

fn main() {
    let holder = Holder {
        name: String::from("moved"),
    };
    let Holder { name } = holder;
    println!("{}", name);
}
