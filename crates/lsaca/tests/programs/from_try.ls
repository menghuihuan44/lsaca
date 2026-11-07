use std::error::Error;

struct AppError {
    message: String,
}

impl AppError {
    fn new(message: String) -> AppError {
        AppError { message: message }
    }
}

impl Error for AppError {
    fn message(&self) -> String {
        self.message.clone()
    }
}

impl From<ParseIntError> for AppError {
    fn from(error: ParseIntError) -> AppError {
        let cause: String = error.message();
        let mut text = String::from("cannot parse: ");
        text.push_str(cause.as_str());
        AppError::new(text)
    }
}

fn parse_number(text: &str) -> Result<i64, AppError> {
    let value: i64 = text.parse_i64()?;
    Ok(value)
}

fn main() {
    match parse_number("42") {
        Ok(value) => println!("ok {}", value),
        Err(error) => println!("err {}", error.message()),
    }
    match parse_number("nope") {
        Ok(value) => println!("ok {}", value),
        Err(error) => println!("err {}", error.message()),
    }
}
