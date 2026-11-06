fn main() {
    // 2^62 elements overflows the allocation size, so the runtime panics
    // with capacity overflow (v2.1: lengths and capacities are usize).
    let values: Vec<i64> = Vec::with_capacity(4611686018427387904);
    println!("{}", values.len());
}