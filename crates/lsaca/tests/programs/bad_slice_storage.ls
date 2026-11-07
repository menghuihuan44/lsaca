// A Vec of slices is still rejected: references cannot be stored in containers.
struct Holder {
    values: Vec<&[i64]>,
}

fn main() {
}
