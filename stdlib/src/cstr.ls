// Borrowed NUL-terminated views (v2.5, std::cstr).
//
// CStr is a Copy view of memory owned by someone else. Constructing one is
// unsafe because the pointer must be valid and NUL-terminated; CStr never
// frees the buffer. to_str() validates UTF-8 and returns a borrowed str tied
// to this view.

use crate::core::Utf8Error;

pub extern fn __str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str;

pub struct CStr {
    ptr: *const u8,
}

impl CStr {
    /// Wraps a C string pointer. The caller promises the pointer is non-null,
    /// valid for reads, and points at a NUL-terminated byte sequence.
    pub unsafe fn from_ptr(ptr: *const u8) -> CStr {
        return CStr { ptr: ptr };
    }

    /// Length in bytes, not counting the terminating NUL. Scans the buffer.
    pub fn len(&self) -> usize {
        let mut index: usize = 0;
        loop {
            let byte: u8 = unsafe { *(self.ptr.add(index)) };
            if byte == 0 {
                return index;
            }
            index = index + 1;
        }
    }

    /// Validates the bytes and returns a borrowed str, or Utf8Error with the
    /// first invalid byte offset.
    pub fn to_str(&self) -> Result<&str, Utf8Error> {
        let length: usize = self.len();
        let valid: usize = __cstr_utf8_check(self.ptr, length);
        if valid != length {
            return Err(Utf8Error::__new(valid));
        }
        return Ok(unsafe { __str_from_raw_parts(self.ptr, length) });
    }
}

// Returns length when the bytes are valid UTF-8, otherwise the index of the
// first byte that starts an invalid sequence.
fn __cstr_utf8_check(ptr: *const u8, length: usize) -> usize {
    let mut index: usize = 0;
    while index < length {
        let first: u8 = unsafe { *(ptr.add(index)) };
        if first < 0x80 {
            index = index + 1;
            continue;
        }
        if first >= 0xC2 && first <= 0xDF {
            if index + 1 >= length {
                return index;
            }
            let second: u8 = unsafe { *(ptr.add(index + 1)) };
            if second < 0x80 || second > 0xBF {
                return index;
            }
            index = index + 2;
            continue;
        }
        if first >= 0xE0 && first <= 0xEF {
            if index + 2 >= length {
                return index;
            }
            let second: u8 = unsafe { *(ptr.add(index + 1)) };
            let third: u8 = unsafe { *(ptr.add(index + 2)) };
            let second_ok: bool = if first == 0xE0 {
                second >= 0xA0 && second <= 0xBF
            } else if first == 0xED {
                second >= 0x80 && second <= 0x9F
            } else {
                second >= 0x80 && second <= 0xBF
            };
            if !second_ok || third < 0x80 || third > 0xBF {
                return index;
            }
            index = index + 3;
            continue;
        }
        if first >= 0xF0 && first <= 0xF4 {
            if index + 3 >= length {
                return index;
            }
            let second: u8 = unsafe { *(ptr.add(index + 1)) };
            let third: u8 = unsafe { *(ptr.add(index + 2)) };
            let fourth: u8 = unsafe { *(ptr.add(index + 3)) };
            let second_ok: bool = if first == 0xF0 {
                second >= 0x90 && second <= 0xBF
            } else if first == 0xF4 {
                second >= 0x80 && second <= 0x8F
            } else {
                second >= 0x80 && second <= 0xBF
            };
            if !second_ok || third < 0x80 || third > 0xBF || fourth < 0x80 || fourth > 0xBF {
                return index;
            }
            index = index + 4;
            continue;
        }
        return index;
    }
    return length;
}
