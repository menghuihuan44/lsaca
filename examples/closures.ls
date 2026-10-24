fn apply_twice(f: fn(i64) -> i64, value: i64) -> i64 {
    f(f(value))
}

fn main() {
    let base = 10;
    let add_base = |x: i64| x + base;
    println!("add_base(5) = {}", add_base(5));
    println!("apply_twice(*2, 3) = {}", apply_twice(|x: i64| x * 2, 3));
}
