// An owned String is not implicitly converted to &str (v2.1).
fn greet(name: &str) {
    println!("{}", name);
}

fn main() {
    let text = String::from("x");
    greet(text);
}
