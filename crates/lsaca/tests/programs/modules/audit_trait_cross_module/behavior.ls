pub trait Speak {
    fn speak(&self) -> i64;
}

pub struct Dog {
    pub value: i64,
}

impl Speak for Dog {
    fn speak(&self) -> i64 {
        self.value
    }
}
