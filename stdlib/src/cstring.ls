// Owned NUL-terminated byte strings (v2.5, std::cstring).
//
// CString owns a Vec<u8> that always ends in a NUL byte. The pointer returned
// by as_ptr() is valid while the CString lives. into_raw() transfers ownership
// of the buffer to the caller; only a pointer from into_raw() may be passed
// back to from_raw(), which rebuilds the owning Vec and frees it normally.

use crate::error::Error as ErrorTrait;
use crate::vec::__slice_slice;
use crate::vec::__vec_from_raw_parts;
use crate::vec::__vec_into_raw_parts;

/// Error returned when a string contains an interior NUL byte.
pub struct NulError {
    position: usize,
}

impl NulError {
    pub fn __new(position: usize) -> NulError {
        return NulError { position: position };
    }

    /// The byte index of the first interior NUL.
    pub fn position(&self) -> usize {
        return self.position;
    }
}

impl ErrorTrait for NulError {
    fn message(&self) -> String {
        return "nul byte found in provided data";
    }
}

pub struct CString {
    bytes: Vec<u8>,
}

impl CString {
    /// Builds a NUL-terminated copy of the text. Returns NulError when the
    /// text contains an interior NUL byte.
    pub fn new(text: &str) -> Result<CString, NulError> {
        let bytes: &[u8] = text.as_bytes();
        let mut index: usize = 0;
        while index < bytes.len() {
            if bytes[index] == 0 {
                return Err(NulError { position: index });
            }
            index = index + 1;
        }
        let mut storage: Vec<u8> = Vec::with_capacity(bytes.len() + 1);
        index = 0;
        while index < bytes.len() {
            storage.push(bytes[index]);
            index = index + 1;
        }
        storage.push(0);
        return Ok(CString { bytes: storage });
    }

    /// Pointer to the first byte of the NUL-terminated buffer.
    pub fn as_ptr(&self) -> *const u8 {
        return self.bytes.as_ptr();
    }

    /// Number of bytes, not counting the trailing NUL.
    pub fn len(&self) -> usize {
        return self.bytes.len() - 1;
    }

    /// The bytes without the trailing NUL.
    pub fn as_bytes(&self) -> &[u8] {
        return __slice_slice(self.bytes.as_slice(), 0, (self.bytes.len() - 1) as i64);
    }

    /// Releases the buffer to the caller. The CString must not be used after
    /// this call, and the returned pointer must eventually be passed back to
    /// from_raw().
    pub fn into_raw(self) -> *mut u8 {
        let CString { bytes } = self;
        let parts: (*mut u8, usize, usize) = unsafe { __vec_into_raw_parts(bytes) };
        return parts.0;
    }

    /// Rebuilds a CString from a pointer produced by into_raw(). The caller
    /// promises that the pointer still owns a NUL-terminated allocation.
    pub unsafe fn from_raw(raw: *mut u8) -> CString {
        let length: usize = __cstring_len(raw as *const u8);
        let bytes: Vec<u8> = __vec_from_raw_parts(raw, length + 1, length + 1);
        return CString { bytes: bytes };
    }
}

// Reads until the first NUL. The caller owns a valid NUL-terminated buffer.
fn __cstring_len(ptr: *const u8) -> usize {
    let mut index: usize = 0;
    loop {
        let byte: u8 = unsafe { *(ptr.add(index)) };
        if byte == 0 {
            return index;
        }
        index = index + 1;
    }
}
