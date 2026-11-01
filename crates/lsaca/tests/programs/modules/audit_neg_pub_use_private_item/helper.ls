// audit: a pub(crate) item cannot be re-exported with pub visibility (E0364).
pub(crate) fn only_in_crate() -> i64 {
    1
}
