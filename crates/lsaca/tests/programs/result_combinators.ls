// Result combinators: Err paths, closure laziness and drop-glue payloads.

fn main() {
    let ok: Result<i64, String> = Ok(4);
    let err: Result<i64, String> = Err(String::from("boom"));

    println!("{} {}", ok.clone().map_or(0, |value| value * 2), err.clone().map_or(0, |value| value * 2));
    println!("{} {}", ok.clone().map_or_else(|error| error.len() as i64, |value| value + 1), err.clone().map_or_else(|error| error.len() as i64, |value| value + 1));
    let primary: Result<i64, String> = err.clone().or(Ok(7));
    println!("{}", primary.unwrap());
    println!("{}", ok.clone().or(Err(String::from("other"))).unwrap());
    let recovered: Result<i64, String> = err.clone().or_else(|error| Ok(error.len() as i64));
    println!("{}", recovered.unwrap());
    println!("{} {}", ok.clone().is_ok_and(|value| value == 4), err.clone().is_ok_and(|value| value == 4));
    println!("{} {}", err.clone().is_err_and(|error| error.len() == 4), ok.clone().is_err_and(|error| error.len() == 4));
    let inspected = ok.clone().inspect(|value| println!("inspect ok {}", *value));
    println!("{}", inspected.unwrap());
    let inspected_err = err.clone().inspect_err(|error| println!("inspect err {}", error.len()));
    println!("{}", inspected_err.unwrap_err());
    println!("{} {}", ok.clone().unwrap_or_else(|error| error.len() as i64), err.clone().unwrap_or_else(|error| error.len() as i64));

    let text_ok: Result<String, String> = Ok(String::from("kept"));
    let text_err: Result<String, String> = Err(String::from("lost"));
    println!("{}", text_ok.clone().map_or(String::from("none"), |value| value.to_uppercase()));
    println!("{}", text_err.clone().map_or(String::from("none"), |value| value.to_uppercase()));
    println!("{}", text_ok.clone().map_or_else(|error| error.to_uppercase(), |value| value.to_uppercase()));
    println!("{}", text_err.clone().map_or_else(|error| error.to_uppercase(), |value| value.to_uppercase()));
    let backup: Result<String, String> = text_err.clone().or(Ok(String::from("backup")));
    println!("{}", backup.unwrap());
    println!("{}", text_ok.clone().or(Err(String::from("other"))).unwrap());
    let recovered_text: Result<String, String> = text_err.clone().or_else(|error| Ok(error.to_uppercase()));
    println!("{}", recovered_text.unwrap());
    println!("{} {}", text_ok.clone().is_ok_and(|value| value.len() == 4), text_err.clone().is_ok_and(|value| value.len() == 4));
    println!("{} {}", text_err.clone().is_err_and(|error| error.len() == 4), text_ok.clone().is_err_and(|error| error.len() == 4));
    let inspected_text = text_ok.clone().inspect(|value| println!("inspect text {}", value.len()));
    println!("{}", inspected_text.unwrap());
    let inspected_text_err = text_err.clone().inspect_err(|error| println!("inspect text err {}", error.len()));
    println!("{}", inspected_text_err.unwrap_err());
    println!("{}", text_ok.unwrap_or_else(|error| error.to_uppercase()));
    println!("{}", text_err.unwrap_or_else(|error| error.to_uppercase()));

    let absent: Option<String> = None;
    match absent.ok_or_else(|| String::from("missing")) {
        Ok(value) => println!("ok {}", value),
        Err(error) => println!("err {}", error),
    }
}
