// System foundation stdlib (v2.5): ptr, mem, cstring, cstr, and buffer pointers.

use std::cstr::CStr;
use std::cstring::CString;
use std::mem::MaybeUninit;
use std::ptr;

struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let null: *const i64 = ptr::null::<i64>();
    let null_mut: *mut i64 = ptr::null_mut::<i64>();
    let another_null: *const i64 = ptr::null::<i64>();
    println!("null {} {}", null.is_null(), null_mut.is_null());
    println!("null eq {}", null == another_null);

    let mut slot: MaybeUninit<i64> = MaybeUninit::<i64>::uninit();
    slot.write(41);
    let value: i64 = unsafe { slot.assume_init() };
    println!("uninit {}", value + 1);

    let mut slot2: MaybeUninit<i64> = MaybeUninit::<i64>::uninit();
    let pointer: *mut i64 = slot2.as_mut_ptr();
    unsafe {
        *pointer = 7;
    }
    let pointed: i64 = unsafe { *pointer };
    let value2: i64 = unsafe { slot2.assume_init() };
    println!("raw write {} {}", pointed, value2);

    let mut point_slot: MaybeUninit<Point> = MaybeUninit::<Point>::uninit();
    point_slot.write(Point { x: 3, y: 4 });
    let point: Point = unsafe { point_slot.assume_init() };
    println!("point {} {}", point.x, point.y);

    let mut text_slot: MaybeUninit<String> = MaybeUninit::<String>::uninit();
    text_slot.write(String::from_str("moved value"));
    let moved: String = unsafe { text_slot.assume_init() };
    println!("moved text {} {}", moved.len(), moved);

    let mut numbers_slot: MaybeUninit<Vec<i64>> = MaybeUninit::<Vec<i64>>::uninit();
    let mut payload: Vec<i64> = Vec::new();
    payload.push(3);
    payload.push(4);
    numbers_slot.write(payload);
    let moved_numbers: Vec<i64> = unsafe { numbers_slot.assume_init() };
    println!("moved vec {} {}", moved_numbers.len(), moved_numbers[1]);

    let text: CString = CString::new("hello").unwrap();
    println!("cstring {} {}", text.len(), !text.as_ptr().is_null());
    let bytes: &[u8] = text.as_bytes();
    println!("bytes {} {}", bytes.len(), bytes[0] as i64);

    let raw: *mut u8 = text.into_raw();
    let rebuilt: CString = unsafe { CString::from_raw(raw) };
    println!("rebuilt {}", rebuilt.len());
    let rebuilt_bytes: &[u8] = rebuilt.as_bytes();
    println!("rebuilt first {}", rebuilt_bytes[0] as i64);

    let empty: CString = CString::new("").unwrap();
    println!("empty {}", empty.len());
    let empty_view: CStr = unsafe { CStr::from_ptr(empty.as_ptr()) };
    println!("empty view {} {}", empty_view.len(), empty_view.to_str().unwrap().len());

    let bad: Result<CString, std::cstring::NulError> = CString::new("ab\0cd");
    match bad {
        Ok(_) => println!("unexpected"),
        Err(error) => println!("nul {}", error.position()),
    }

    let greek: CString = CString::new("héllo").unwrap();
    let view: CStr = unsafe { CStr::from_ptr(greek.as_ptr()) };
    println!("view {} {}", view.len(), view.to_str().unwrap());

    let mut invalid: Vec<u8> = Vec::new();
    invalid.push(0x61);
    invalid.push(0xFF);
    invalid.push(0);
    let bad_view: CStr = unsafe { CStr::from_ptr(invalid.as_ptr()) };
    match bad_view.to_str() {
        Ok(_) => println!("unexpected"),
        Err(error) => println!("utf8 {}", error.valid_up_to()),
    }

    let owned: String = String::from_str("lsaca");
    let owned_ptr: *const u8 = owned.as_ptr();
    let first: u8 = unsafe { *owned_ptr };
    println!("string {} {}", first as i64, owned.len());

    let empty_text: String = String::from_str("");
    println!("empty string {}", empty_text.as_ptr().is_null());

    let mut numbers: Vec<i64> = Vec::new();
    numbers.push(5);
    numbers.push(6);
    let numbers_ptr: *mut i64 = numbers.as_mut_ptr();
    let first_number: i64 = unsafe { *numbers_ptr };
    let read_ptr: *const i64 = numbers.as_ptr();
    let again: i64 = unsafe { *read_ptr };
    println!("vec {} {} {}", first_number, again, numbers.len());

    let mut empty_numbers: Vec<i64> = Vec::new();
    println!(
        "empty vec {} {}",
        empty_numbers.as_ptr().is_null(),
        empty_numbers.as_mut_ptr().is_null()
    );
}
