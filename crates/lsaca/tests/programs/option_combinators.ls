// Option combinators: None paths, closure laziness and drop-glue payloads.

fn main() {
    let some: Option<i64> = Some(5);
    let none: Option<i64> = None;

    println!("{} {}", some.map_or_else(|| -1, |value| value * 2), none.map_or_else(|| -1, |value| value * 2));
    println!("{}", some.or_else(|| None).unwrap());
    println!("{}", none.or_else(|| Some(7)).unwrap());
    println!("{} {}", some.unwrap_or_else(|| 9), none.unwrap_or_else(|| 9));
    println!("{} {}", some.is_some_and(|value| value > 3), none.is_some_and(|value| value > 3));
    println!("{} {}", some.is_none_or(|value| value > 10), none.is_none_or(|value| value > 10));
    println!("{} {}", some.and(Some(6)).unwrap(), none.and(Some(6)).unwrap_or(-1));

    let text: Option<String> = Some(String::from("lsaca"));
    let absent: Option<String> = None;

    println!("{}", text.clone().map_or_else(|| String::from("none"), |value| value.to_uppercase()));
    println!("{}", absent.clone().map_or_else(|| String::from("none"), |value| value.to_uppercase()));
    match text.clone().or_else(|| None) {
        Some(value) => println!("{}", value),
        None => println!("none"),
    }
    match absent.clone().or_else(|| Some(String::from("fallback"))) {
        Some(value) => println!("{}", value),
        None => println!("none"),
    }
    println!("{}", text.clone().unwrap_or_else(|| String::from("unused")));
    println!("{}", absent.clone().unwrap_or_else(|| String::from("fresh")));
    println!("{} {}", text.clone().is_some_and(|value| value.len() == 5), absent.clone().is_some_and(|value| value.len() == 5));
    println!("{} {}", absent.clone().is_none_or(|value| value.len() == 5), text.clone().is_none_or(|value| value.len() == 99));
    match text.clone().and(Some(String::from("joined"))) {
        Some(value) => println!("{}", value),
        None => println!("none"),
    }
    match absent.clone().and(Some(String::from("joined"))) {
        Some(value) => println!("{}", value),
        None => println!("none"),
    }
    match absent.clone().ok_or_else(|| String::from("missing")) {
        Ok(value) => println!("ok {}", value),
        Err(error) => println!("err {}", error),
    }
    match text.ok_or_else(|| String::from("missing")) {
        Ok(value) => println!("ok {}", value),
        Err(error) => println!("err {}", error),
    }
}
