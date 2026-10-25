fn main() {
    let name: Option<String> = Some(String::from("lsaca"));
    match name.filter(|text| text.len() > 2) {
        Some(text) => println!("kept {}", text),
        None => println!("dropped"),
    }
    let short: Option<String> = Some(String::from("a"));
    match short.filter(|text| text.len() > 2) {
        Some(text) => println!("kept {}", text),
        None => println!("dropped"),
    }
    let chain = Some(3)
        .and_then(|value| {
            if value > 2 { Some(value * 10) } else { None }
        })
        .and_then(|value| Some(value + 1));
    println!("{}", chain.unwrap());
    let result: Result<i64, String> = Ok(5);
    let next = result.and_then(|value| Ok(value * 3));
    println!("{}", next.unwrap());
    let failed: Result<i64, String> = Err(String::from("bad"));
    match failed.and_then(|value| Ok(value + 1)) {
        Ok(value) => println!("ok {}", value),
        Err(message) => println!("err {}", message),
    }
    let captured = 4;
    let scaled = Some(5).map(|value| value * captured);
    println!("{}", scaled.unwrap());
}
