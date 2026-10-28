// Moving a non-Copy field out of a struct is rejected (E0507); clone instead.
struct Config {
    name: String,
}

fn main() {
    let config = Config { name: String::from("hi") };
    let name = config.name;
    println!("{}", name);
}
