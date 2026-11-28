union Number {
    integer: i64,
    float: f64,
}

fn main() {
    let number = unsafe { Number { integer: 1 } };
    match number {
        Number { integer } => println!("{}", integer),
    }
}
