// repr(C) on a union is an accepted no-op: unions already have C layout.
#[repr(C)]
union Bits {
    integer: i64,
    float: f64,
}

fn main() {
    let bits: Bits = unsafe { Bits { integer: 4607182418800017408 } };
    let value: f64 = unsafe { bits.float };
    println!("repr union {:.1}", value);
}
