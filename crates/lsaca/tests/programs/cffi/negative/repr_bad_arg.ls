#[repr(align(8))]
struct Aligned {
    value: i64,
}

fn main() {
    let aligned = Aligned { value: 1 };
    println!("{}", aligned.value);
}
