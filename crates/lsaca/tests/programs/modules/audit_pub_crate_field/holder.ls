pub(crate) struct Counter {
    pub(crate) count: i64,
}

pub(crate) fn make() -> Counter {
    Counter { count: 2 }
}
