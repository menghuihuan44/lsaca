fn min_max(values: Vec<i64>) -> (i64, i64) {
    let mut lo = values[0];
    let mut hi = values[0];
    for v in values {
        if v < lo {
            lo = v;
        }
        if v > hi {
            hi = v;
        }
    }
    (lo, hi)
}

fn main() {
    let pair = min_max(vec![4, 1, 9, 3]);
    println!("{} {}", pair.0, pair.1);
    let (a, b) = pair;
    println!("{} {}", a, b);
}
