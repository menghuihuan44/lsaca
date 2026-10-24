fn check(value: Option<i64>) -> i64 {
    match value {
        Some(n) => n,
    }
}

fn main() {
    println!("{}", check(Some(1)));
}
