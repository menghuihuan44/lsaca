// Mutable parameters and string concatenation with &str (v2.1).
fn bump(mut value: i64) -> i64 {
    value += 1;
    value
}

fn fill(mut text: String, suffix: String) -> String {
    text += "!";
    text + &suffix
}

fn main() {
    let x = 10;
    println!("{} {}", x, bump(x));

    let mut s = String::from("hi");
    s += " there";
    println!("{}", fill(s, String::from(" bye")));
}