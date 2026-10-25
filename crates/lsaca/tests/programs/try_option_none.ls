// None propagates from a callee through the try operator.
fn positive(value: i64) -> Option<i64> {
    if value <= 0 {
        return None;
    }
    Some(value)
}

fn doubled(value: i64) -> Option<i64> {
    let number = positive(value)?;
    Some(number * 2)
}

fn show(result: Option<i64>) {
    match result {
        Some(value) => println!("got {}", value),
        None => println!("no value"),
    }
}

fn main() {
    show(doubled(21));
    show(doubled(-4));
}