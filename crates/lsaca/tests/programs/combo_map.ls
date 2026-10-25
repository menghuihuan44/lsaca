fn main() {
    let value: Option<i64> = Some(21);
    let doubled = value.map(|number| number * 2);
    println!("{}", doubled.unwrap());
    let missing: Option<i64> = None;
    println!("{}", missing.map(|number| number * 2).unwrap_or(-1));
    let fallback = missing.map_or(7, |number| number + 1);
    println!("{}", fallback);
    let even = value.filter(|number| number % 2 == 0);
    println!("{}", even.is_some());
    let error = missing.ok_or(String::from("nothing"));
    println!("{}", error.is_err());
    let parsed: Result<i64, String> = "21".parse_i64();
    let bumped = parsed.map(|number| number + 1);
    println!("{}", bumped.unwrap());
    let broken: Result<i64, String> = "x".parse_i64();
    let described = broken.map_err(|message| message.to_uppercase());
    match described {
        Ok(number) => println!("ok {}", number),
        Err(message) => println!("err {}", message),
    }
}
