// The try operator forwards the Ok payload and any Err value.
fn parse_plus(text: String) -> Result<i64, String> {
    let value = text.parse_i64()?;
    Ok(value + 1)
}

fn main() {
    println!("{}", parse_plus("41").unwrap());
    println!("{}", parse_plus("9").unwrap());
}