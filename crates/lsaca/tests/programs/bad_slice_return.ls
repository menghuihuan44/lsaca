// A borrowed slice cannot be returned without a lifetime from a single owner (v2.1).
fn take() -> &[i64] {
    let values = vec![1, 2, 3];
    &values
}

fn main() {
    println!("{}", take().len());
}
