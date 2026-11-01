pub(crate) fn hidden() -> i64 {
    5
}

pub fn visible() -> i64 {
    6
}

pub struct Holder {
    pub(crate) value: i64,
    pub label: String,
}

pub fn holder() -> Holder {
    Holder {
        value: 7,
        label: String::from("held"),
    }
}
