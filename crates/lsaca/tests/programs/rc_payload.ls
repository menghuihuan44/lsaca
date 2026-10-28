// Rc payloads own their data (strings and vectors).
struct Config {
    name: String,
    values: Vec<i64>,
}

fn main() {
    let config = Rc::new(Config {
        name: String::from("default"),
        values: vec![1, 2, 3],
    });

    let copy = config.clone();
    let snapshot = copy.get();
    println!("{} {:?}", config.get().name, snapshot.values);
    println!("{}", copy.get().values.len());

    let text = Rc::new(String::from("hello"));
    let text2 = text.clone();
    println!("{}", text.get());
    text2.set(String::from("world"));
    println!("{}", text.get());
}
