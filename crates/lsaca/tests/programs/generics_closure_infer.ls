// An unannotated closure passed to a callable generic bound is inferred from
// the bound: the bound supplies the closure parameter and return types.

fn apply_twice<F: Fn(i64) -> i64>(f: F, value: i64) -> i64 {
    return f(f(value));
}

fn apply_value<T, F: Fn(T) -> T>(value: T, f: F) -> T {
    return f(value);
}

fn main() {
    println!("{}", apply_twice(|x| x * 2, 3));
    println!("{}", apply_value(4, |x| x + 1));
}
