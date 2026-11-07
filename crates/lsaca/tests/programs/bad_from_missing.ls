// The question-mark operator needs a From conversion (v2.2).
use std::error::Error;

struct AppError {
    message: String,
}

impl Error for AppError {
    fn message(&self) -> String {
        self.message.clone()
    }
}

fn parse(text: &str) -> Result<i64, AppError> {
    let value: i64 = text.parse_i64()?;
    return Ok(value);
}

fn main() {
    match parse("42") {
        Ok(value) => println!("{}", value),
        Err(error) => println!("{}", error.message()),
    }
}
