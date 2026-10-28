fn dangling() -> &i64 {
    let local = 1;
    &local
}

fn main() {}
