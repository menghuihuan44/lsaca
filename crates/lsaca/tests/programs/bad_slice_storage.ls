// References and slices cannot be stored in user types (v2.1).
struct Holder {
    values: &[i64],
}

fn main() {
}
