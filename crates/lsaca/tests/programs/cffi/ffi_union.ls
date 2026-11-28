// A union overlays an i64 and an f64. Both construction and reads are unsafe.
union Number {
    integer: i64,
    float: f64,
}

fn main() {
    let one: Number = unsafe { Number { integer: 4607182418800017408 } };
    let as_float: f64 = unsafe { one.float };
    println!("bits to float {:.1}", as_float);

    let two_and_a_half: Number = unsafe { Number { float: 2.5 } };
    let as_integer: i64 = unsafe { two_and_a_half.integer };
    println!("float to bits {}", as_integer);
}
