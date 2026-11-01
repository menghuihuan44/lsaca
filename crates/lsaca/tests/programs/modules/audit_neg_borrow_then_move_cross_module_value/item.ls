pub struct Item {
    pub value: i64,
    pub label: String,
}

pub fn build(value: i64) -> Item {
    Item { value: value, label: String::from("item") }
}
