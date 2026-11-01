mod child;

fn secret() -> i64 {
    3
}

pub fn call() -> i64 {
    child::alt()
}
