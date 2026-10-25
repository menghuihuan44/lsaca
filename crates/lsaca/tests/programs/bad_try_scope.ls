// The try operator needs an enclosing function that returns Option or Result,
// so it cannot be used inside main, which returns ().
fn maybe(value: i64) -> Option<i64> {
    Some(value)
}

fn main() {
    let value = maybe(1)?;
    println!("{}", value);
}