pub fn greet(name: String) -> String {
    String::from("hello, ") + &name
}

pub const ANSWER: i64 = 42;

pub struct Counter {
    pub value: i64,
}

pub fn counter() -> Counter {
    Counter { value: 7 }
}

pub struct Secret {
    hidden: i64,
}

pub fn secret() -> Secret {
    Secret { hidden: 13 }
}
