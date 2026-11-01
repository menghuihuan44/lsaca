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
    let wrapper = Wrapper::Text(String::from("payload"));
    match wrapper {
        Wrapper::Text(text) => println!("{}", text),
        Wrapper::Number(value) => println!("{}", value),
    }
}
