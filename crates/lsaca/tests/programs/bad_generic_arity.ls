// Wrong number of generic arguments must be rejected (E0107).
struct Pair<T> {
    first: T,
    second: T,
}

fn main() {
    let broken: Pair<i64, bool> = Pair { first: 1, second: 2 };
    println!("{}", broken.first);
}
