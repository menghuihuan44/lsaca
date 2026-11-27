pub(in crate::helper) fn restricted() -> i64 {
    2
}

pub fn deep_value() -> i64 {
    restricted() * 5 + 1
}
