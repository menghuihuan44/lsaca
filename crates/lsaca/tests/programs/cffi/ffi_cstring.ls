// CString owns a NUL-terminated copy; CStr is a borrowed view created from a
// raw pointer. into_raw/from_raw move ownership through the raw pointer.
use std::cstring::CString;
use std::cstr::CStr;

fn main() {
    let text: CString = CString::new("lsaca").unwrap();
    println!("cstring len {}", text.len());

    let view: CStr = unsafe { CStr::from_ptr(text.as_ptr()) };
    let decoded: &str = view.to_str().unwrap();
    println!("cstr {}", decoded);

    let raw: *mut u8 = text.into_raw();
    let back: CString = unsafe { CString::from_raw(raw) };
    println!("roundtrip {}", back.len());
}
