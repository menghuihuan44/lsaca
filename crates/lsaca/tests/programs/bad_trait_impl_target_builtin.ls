// A trait can only be implemented for a struct or enum defined in the
// program. Implementing a trait for a builtin type must be rejected.

trait Tag {
    fn tag(&self) -> i64;
}

impl Tag for Vec<i64> {
    fn tag(&self) -> i64 {
        return 1;
    }
}

fn main() {
    let values: Vec<i64> = vec![1, 2];
    println!("{}", values.len());
}
