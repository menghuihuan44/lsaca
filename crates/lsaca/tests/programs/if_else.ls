fn classify(n: i64) -> String {
    if n < 0 {
        String::from("negative")
    } else if n == 0 {
        String::from("zero")
    } else {
        String::from("positive")
    }
}

fn main() {
    println!("{} {} {}", classify(-3), classify(0), classify(9));
    let x = if 2 > 1 { 10 } else { 20 };
    println!("{}", x);
}
