// Function values are move-only: taking one out of a struct field is E0507.
struct Handler {
    apply: fn(i64) -> i64,
}

fn double(value: i64) -> i64 {
    return value * 2;
}

fn main() {
    let handler = Handler { apply: double };
    let function = handler.apply;
    println!("{}", function(3));
}
