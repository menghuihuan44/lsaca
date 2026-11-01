struct Pair {
    a: i64,
    b: i64,
}

impl Drop for Pair {
    fn drop(&mut self) {
        println!("drop pair");
    }
}

fn main() {
    let pair = Pair { a: 1, b: 2 };
    let Pair { a, b } = pair;
    // Copy bindings do not move the aggregate: the original stays alive and
    // its Drop hook runs exactly once, at the end of the scope.
    println!("{} {} {}", a, b, pair.a);
}
