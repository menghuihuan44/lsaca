// MaybeUninit storage (v2.5).

use std::mem::MaybeUninit;

struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let mut slot: MaybeUninit<i64> = MaybeUninit::uninit();
    slot.write(7);
    let number: i64 = unsafe { slot.assume_init() };
    println!("{}", number);

    let mut point_slot: MaybeUninit<Point> = MaybeUninit::uninit();
    point_slot.write(Point { x: 3, y: 4 });
    let point: Point = unsafe { point_slot.assume_init() };
    println!("{} {}", point.x, point.y);

    let mut raw_slot: MaybeUninit<i64> = MaybeUninit::uninit();
    let target: *mut i64 = raw_slot.as_mut_ptr();
    unsafe {
        *target = 12;
    }
    println!("{}", unsafe { raw_slot.assume_init() });
}
