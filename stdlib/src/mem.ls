// Uninitialized storage (v2.5, std::mem).
//
// MaybeUninit<T> is a builtin layout wrapper: its size and alignment are
// exactly those of T, including when T needs drop glue. Writing a value and
// then abandoning the wrapper leaks that value; it is never a double free.
// Reading an initialized value requires unsafe and the unsafe assume_init
// call, which consumes the wrapper.

pub struct MaybeUninit<T> {
    internal: (),
}

pub extern fn __maybe_uninit_uninit<T>() -> MaybeUninit<T>;
pub extern fn __maybe_uninit_write<T>(slot: &mut MaybeUninit<T>, value: T);
pub extern fn __maybe_uninit_as_mut_ptr<T>(slot: &mut MaybeUninit<T>) -> *mut T;
pub extern fn __maybe_uninit_assume_init<T>(slot: &mut MaybeUninit<T>) -> T;

impl<T> MaybeUninit<T> {
    /// Creates a value whose contents are uninitialized.
    pub fn uninit() -> MaybeUninit<T> {
        return __maybe_uninit_uninit::<T>();
    }

    /// Writes a value into the slot. Any previous value is overwritten without
    /// being dropped, so writing twice leaks the first value; use the slot as
    /// storage for at most one live value.
    pub fn write(&mut self, value: T) {
        __maybe_uninit_write(self, value);
    }

    /// Returns a mutable raw pointer to the storage, suitable for FFI output
    /// parameters.
    pub fn as_mut_ptr(&mut self) -> *mut T {
        return __maybe_uninit_as_mut_ptr(self);
    }

    /// Reads the value out of the slot. The caller must guarantee that a value
    /// was initialized first; the wrapper is consumed and its contents are
    /// moved into the result.
    pub unsafe fn assume_init(self) -> T {
        let mut slot: MaybeUninit<T> = self;
        return __maybe_uninit_assume_init(&mut slot);
    }
}
