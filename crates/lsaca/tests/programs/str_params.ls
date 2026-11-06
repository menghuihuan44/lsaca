// Read-only text parameters are &str; literals convert without allocating (v2.1).
fn greet(name: &str) -> String {
    String::from_str("hello, ") + name
}

fn shout(mut text: String) -> String {
    text += "!";
    text
}

fn describe(name: &str, times: usize) -> String {
    let mut result = String::new();
    let mut i: usize = 0;
    while i < times {
        result += name;
        i += 1;
    }
    result
}

fn main() {
    println!("{}", greet("world"));
    let name = String::from("lsaca");
    println!("{}", greet(&name));
    println!("{}", shout(greet("rust")));
    println!("{}", describe("ab", 3));

    let mut text = String::from("hey");
    text += " you";
    println!("{}", text);
    println!("{}", String::from("x") + "y");
}