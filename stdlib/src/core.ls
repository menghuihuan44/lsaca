// Core standard-library types: Option<T>, Result<T, E>, Rc<T> and Box<T>.
//
// The compiler knows these as builtin types; each marker struct below only
// claims the name for the standard library. Methods are inherent impls backed
// by extern intrinsics, mirroring the legacy builtin signatures exactly.

pub struct Option<T> {
    internal: (),
}

pub extern fn __opt_some<T>(value: T) -> Option<T>;
pub extern fn __opt_none<T>() -> Option<T>;
pub extern fn __opt_is_some<T>(option: &Option<T>) -> bool;
pub extern fn __opt_is_none<T>(option: &Option<T>) -> bool;
pub extern fn __opt_unwrap<T>(option: Option<T>) -> T;
pub extern fn __opt_unwrap_or<T>(option: Option<T>, fallback: T) -> T;
pub extern fn __opt_expect<T>(option: Option<T>, message: String) -> T;

impl<T> Option<T> {
    pub fn is_some(&self) -> bool {
        return __opt_is_some(self);
    }

    pub fn is_none(&self) -> bool {
        return __opt_is_none(self);
    }

    pub fn unwrap(self) -> T {
        return __opt_unwrap(self);
    }

    pub fn unwrap_or(self, fallback: T) -> T {
        return __opt_unwrap_or(self, fallback);
    }

    pub fn expect(self, message: String) -> T {
        return __opt_expect(self, message);
    }
}

pub struct Result<T, E> {
    internal: (),
}

pub extern fn __res_ok<T, E>(value: T) -> Result<T, E>;
pub extern fn __res_err<T, E>(value: E) -> Result<T, E>;
pub extern fn __res_is_ok<T, E>(result: &Result<T, E>) -> bool;
pub extern fn __res_is_err<T, E>(result: &Result<T, E>) -> bool;
pub extern fn __res_unwrap<T, E>(result: Result<T, E>) -> T;
pub extern fn __res_unwrap_or<T, E>(result: Result<T, E>, fallback: T) -> T;
pub extern fn __res_expect<T, E>(result: Result<T, E>, message: String) -> T;
pub extern fn __res_ok_value<T, E>(result: Result<T, E>) -> Option<T>;
pub extern fn __res_err_value<T, E>(result: Result<T, E>) -> Option<E>;

impl<T, E> Result<T, E> {
    pub fn is_ok(&self) -> bool {
        return __res_is_ok(self);
    }

    pub fn is_err(&self) -> bool {
        return __res_is_err(self);
    }

    pub fn unwrap(self) -> T {
        return __res_unwrap(self);
    }

    pub fn unwrap_or(self, fallback: T) -> T {
        return __res_unwrap_or(self, fallback);
    }

    pub fn expect(self, message: String) -> T {
        return __res_expect(self, message);
    }

    pub fn ok(self) -> Option<T> {
        return __res_ok_value(self);
    }

    pub fn err(self) -> Option<E> {
        return __res_err_value(self);
    }
}

pub struct Rc<T> {
    internal: (),
}

pub extern fn __rc_new<T>(value: T) -> Rc<T>;
pub extern fn __rc_get<T>(cell: &Rc<T>) -> T;
pub extern fn __rc_set<T>(cell: &Rc<T>, value: T);
pub extern fn __rc_strong_count<T>(cell: &Rc<T>) -> i64;

impl<T> Rc<T> {
    pub fn new(value: T) -> Rc<T> {
        return __rc_new(value);
    }

    pub fn get(&self) -> T {
        return __rc_get(self);
    }

    pub fn set(&self, value: T) {
        __rc_set(self, value);
    }

    pub fn strong_count(&self) -> i64 {
        return __rc_strong_count(self);
    }
}

pub struct Box<T> {
    internal: (),
}

pub extern fn __box_new<T>(value: T) -> Box<T>;
pub extern fn __box_get<T>(cell: &Box<T>) -> T;
pub extern fn __box_set<T>(cell: &mut Box<T>, value: T);

impl<T> Box<T> {
    pub fn new(value: T) -> Box<T> {
        return __box_new(value);
    }

    pub fn get(&self) -> T {
        return __box_get(self);
    }

    pub fn set(&mut self, value: T) {
        __box_set(self, value);
    }
}