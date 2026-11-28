// Real libc bindings: strlen, strcmp, memcmp and getenv. The calls are
// unsafe because they leave the safety of lsaca; the pointers come from
// CString values that outlive each call.
use std::cstring::CString;
use std::cstr::CStr;
use std::ptr::null;

extern "C" {
    fn strlen(text: *const u8) -> usize;
    fn strcmp(left: *const u8, right: *const u8) -> i32;
    fn memcmp(left: *const u8, right: *const u8, count: usize) -> i32;
    fn getenv(name: *const u8) -> *const u8;
}

fn main() {
    let alpha: CString = CString::new("alpha").unwrap();
    let beta: CString = CString::new("beta").unwrap();
    let absent_name: CString = CString::new("LSACA_FFI_SUITE_ABSENT").unwrap();
    let path_name: CString = CString::new("PATH").unwrap();

    let length: usize = unsafe { strlen(alpha.as_ptr()) };
    println!("strlen {}", length);

    let equal: bool = unsafe { strcmp(alpha.as_ptr(), alpha.as_ptr()) == 0 };
    println!("strcmp equal {}", equal);

    let less: bool = unsafe { strcmp(alpha.as_ptr(), beta.as_ptr()) < 0 };
    println!("strcmp less {}", less);

    let same: bool = unsafe { memcmp(alpha.as_ptr(), alpha.as_ptr(), 5) == 0 };
    println!("memcmp equal {}", same);

    let differs: bool = unsafe { memcmp(alpha.as_ptr(), beta.as_ptr(), 1) < 0 };
    println!("memcmp less {}", differs);

    let view: CStr = unsafe { CStr::from_ptr(alpha.as_ptr()) };
    let decoded: &str = view.to_str().unwrap();
    println!("cstr {} {}", view.len(), decoded);

    let nothing: *const u8 = null::<u8>();
    println!("null {}", nothing.is_null());

    let missing: *const u8 = unsafe { getenv(absent_name.as_ptr()) };
    println!("missing null {}", missing.is_null());

    let path: *const u8 = unsafe { getenv(path_name.as_ptr()) };
    println!("path non-null {}", !path.is_null());
}
