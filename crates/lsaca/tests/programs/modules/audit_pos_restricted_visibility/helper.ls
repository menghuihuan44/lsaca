pub(super) fn value() -> i64 {
    1
}

pub(crate) fn crate_wide() -> i64 {
    inner::deep_value()
}

mod inner;
