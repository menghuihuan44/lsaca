// Unions are read and written through unsafe (v2.5).

#[repr(C)]
union Word {
    as_integer: i64,
    as_float: f64,
}

#[repr(C)]
struct Packed {
    tag: i64,
    payload: Word,
}

fn main() {
    let mut word: Word = unsafe { Word { as_integer: 0 } };
    unsafe {
        word.as_integer = 1099511627776;
    }
    println!("{}", unsafe { word.as_integer });
    let float_word: Word = unsafe { Word { as_float: 0.5 } };
    println!("{}", unsafe { float_word.as_float });
    unsafe {
        let packed: Packed = Packed { tag: 1, payload: Word { as_float: 2.5 } };
        println!("{}", packed.payload.as_float);
    }
}
