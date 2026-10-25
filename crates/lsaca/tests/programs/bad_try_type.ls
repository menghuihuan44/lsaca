// The try operator only accepts Option and Result values.
fn compute(value: i64) -> Option<i64> {
    let number = value?;
    Some(number)
}

fn main() {
    match compute(1) {
        Some(value) => println!("{}", value),
        None => println!("none"),
    }
}