#[repr(C)]
enum Kind {
    First,
    Second,
}

fn main() {
    let kind = Kind::First;
    println!("unreachable");
}
