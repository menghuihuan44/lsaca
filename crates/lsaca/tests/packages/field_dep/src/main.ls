use greeting::{counter, greet, Counter};

fn main() {
    let counted: Counter = counter();
    println!("{}", counted.value);
    println!("{}", greet(String::from("field")));
}
