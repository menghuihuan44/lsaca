// Raw pointers and unsafe blocks (v2.5).

fn bump(ptr: *mut i64) -> i64 {
    unsafe {
        *ptr = *ptr + 1;
        return *ptr;
    }
}

unsafe fn read(ptr: *const i64) -> i64 {
    return *ptr;
}

fn main() {
    let mut value: i64 = 41;
    let mutable: *mut i64 = &mut value as *mut i64;
    let shared: *const i64 = &value as *const i64;
    println!("{}", bump(mutable));
    println!("{}", value);
    println!("{}", unsafe { read(shared) });
    println!("{}", mutable.is_null());
    println!("{}", (0 as *const i64).is_null());
    unsafe {
        println!("{}", *mutable == 42);
    }
}
