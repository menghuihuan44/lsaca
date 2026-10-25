// A propagated Err keeps its original error payload.
fn parse_twice(text: String) -> Result<i64, String> {
    let value = text.parse_i64()?;
    Ok(value * 2)
}

fn main() {
    match parse_twice("21") {
        Ok(value) => println!("ok {}", value),
        Err(message) => println!("err {}", message),
    }
    match parse_twice("nope") {
        Ok(value) => println!("ok {}", value),
        Err(message) => println!("err {}", message),
    }
}