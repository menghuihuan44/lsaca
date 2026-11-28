union Number {
    integer: i64,
}

fn main() {
    let number = unsafe { Number { integer: 1 } };
    number.value();
}
