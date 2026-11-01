use greeting::{greet, ANSWER};

fn main() {
    println!("{}", greet(String::from("package")));
    println!("{}", ANSWER);
}
