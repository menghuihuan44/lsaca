fn apply(f: fn(i64, i64) -> i64, a: i64, b: i64) -> i64 {
    f(a, b)
}

fn main() {
    let add = |a: i64, b: i64| a + b;
    let mul = |a: i64, b: i64| a * b;
    println!("{} {}", apply(add, 2, 3), apply(mul, 2, 3));
    let base = 10;
    let plus_base = |x: i64| x + base;
    println!("{}", plus_base(5));
}
