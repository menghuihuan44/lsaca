// Function values are move-only: a function moved into a struct literal cannot
// be used again, so the later call is E0382.
struct Handler {
    apply: fn(i64) -> i64,
}

fn double(value: i64) -> i64 {
    return value * 2;
}

fn main() {
    let apply: fn(i64) -> i64 = double;
    let handler = Handler { apply: apply };
    println!("{}", apply(1));
    println!("{}", (handler.apply)(2));
}
