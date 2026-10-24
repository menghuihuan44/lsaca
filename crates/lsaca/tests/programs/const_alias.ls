const MAX: i64 = 100;
type Number = i64;

fn double(n: Number) -> Number {
    n * 2
}

fn main() {
    println!("{}", double(MAX));
}
