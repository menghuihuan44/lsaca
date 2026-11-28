// Raw pointer helpers (v2.5, std::ptr).
//
// Only null construction lives here: pointer arithmetic (add/sub) and raw
// dereferences are compiler operations that must appear inside unsafe.

/// Returns a null const pointer.
pub fn null<T>() -> *const T {
    return 0 as *const T;
}

/// Returns a null mutable pointer.
pub fn null_mut<T>() -> *mut T {
    return 0 as *mut T;
}
