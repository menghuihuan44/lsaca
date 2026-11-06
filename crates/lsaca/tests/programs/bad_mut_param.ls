// Parameters are immutable unless declared mut (v2.1).
fn bump(value: i64) -> i64 {
    value += 1;
    value
}

fn main() {
    println!("{}", bump(1));
}
