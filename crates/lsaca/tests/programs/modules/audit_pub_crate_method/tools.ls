pub(crate) struct Tool {
    value: i64,
}

impl Tool {
    pub(crate) fn new(value: i64) -> Tool {
        Tool { value }
    }

    pub(crate) fn value(&self) -> i64 {
        self.value
    }
}

pub(crate) fn twice(value: i64) -> i64 {
    value * 2
}
