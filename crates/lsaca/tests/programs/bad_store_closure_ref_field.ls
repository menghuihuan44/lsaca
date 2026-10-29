// A closure that captures a reference cannot be stored in a struct field (E0498).
struct Stored {
    read: fn() -> i64,
}

fn main() {
    let number = 1;
    let reference = &number;
    let read = || *reference;
    let stored = Stored { read: read };
    println!("{}", (stored.read)());
}
