// Vec<T>: growable array.
//
// The compiler knows Vec<T> as a builtin type; this declaration only claims the
// name for the standard library. The methods below are inherent impls backed by
// extern intrinsics, so codegen expands them inline and the legacy compiler
// fallback table no longer participates for these names.

pub struct Vec<T> {
    internal: (),
}

// Intrinsic declarations. Parameter lists mirror the legacy builtin signatures
// exactly: the first parameter is the receiver place, the rest are arguments.
pub extern fn __vec_new<T>() -> Vec<T>;
pub extern fn __vec_with_capacity<T>(capacity: i64) -> Vec<T>;
pub extern fn __vec_len<T>(v: &Vec<T>) -> i64;
pub extern fn __vec_is_empty<T>(v: &Vec<T>) -> bool;
pub extern fn __vec_push<T>(v: &mut Vec<T>, value: T);
pub extern fn __vec_pop<T>(v: &mut Vec<T>) -> Option<T>;
pub extern fn __vec_insert<T>(v: &mut Vec<T>, index: i64, value: T);
pub extern fn __vec_remove<T>(v: &mut Vec<T>, index: i64) -> T;
pub extern fn __vec_clear<T>(v: &mut Vec<T>);
pub extern fn __vec_first<T>(v: &Vec<T>) -> Option<T>;
pub extern fn __vec_last<T>(v: &Vec<T>) -> Option<T>;
pub extern fn __vec_get<T>(v: &Vec<T>, index: i64) -> Option<T>;
pub extern fn __vec_extend<T>(v: &mut Vec<T>, other: Vec<T>);
pub extern fn __vec_reverse<T>(v: &mut Vec<T>);

impl<T> Vec<T> {
    pub fn new() -> Vec<T> {
        return __vec_new::<T>();
    }

    pub fn with_capacity(capacity: i64) -> Vec<T> {
        return __vec_with_capacity::<T>(capacity);
    }

    pub fn len(&self) -> i64 {
        return __vec_len(self);
    }

    pub fn is_empty(&self) -> bool {
        return __vec_is_empty(self);
    }

    pub fn push(&mut self, value: T) {
        __vec_push(self, value);
    }

    pub fn pop(&mut self) -> Option<T> {
        return __vec_pop(self);
    }

    pub fn insert(&mut self, index: i64, value: T) {
        __vec_insert(self, index, value);
    }

    pub fn remove(&mut self, index: i64) -> T {
        return __vec_remove(self, index);
    }

    pub fn clear(&mut self) {
        __vec_clear(self);
    }

    pub fn first(&self) -> Option<T> {
        return __vec_first(self);
    }

    pub fn last(&self) -> Option<T> {
        return __vec_last(self);
    }

    pub fn get(&self, index: i64) -> Option<T> {
        return __vec_get(self, index);
    }

    pub fn extend(&mut self, other: Vec<T>) {
        __vec_extend(self, other);
    }

    pub fn reverse(&mut self) {
        __vec_reverse(self);
    }

    /// Sorts the vector in ascending order in place (shell sort).
    ///
    /// This is written in lsaca itself, so it works for every element type that
    /// supports the comparison operators, including `i64`, `f64` and `String`.
    /// Stability is not guaranteed (equal elements may be reordered).
    pub fn sort(&mut self) {
        let length: i64 = __vec_len::<T>(self);
        let mut gap: i64 = length / 2;
        while gap > 0 {
            let mut index: i64 = gap;
            while index < length {
                let value: T = __vec_get::<T>(self, index).unwrap();
                let mut target: i64 = index;
                let mut moving: bool = true;
                while moving {
                    if target < gap {
                        moving = false;
                    } else {
                        let previous: T = __vec_get::<T>(self, target - gap).unwrap();
                        if previous > value {
                            target = target - gap;
                        } else {
                            moving = false;
                        }
                    }
                }
                if target != index {
                    __vec_remove::<T>(self, index);
                    __vec_insert::<T>(self, target, value);
                }
                index = index + 1;
            }
            gap = gap / 2;
        }
    }

    pub fn as_slice(&self) -> &[T] {
        return __vec_as_slice(self);
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        return __vec_as_mut_slice(self);
    }
}

// Frozen Phase B slice intrinsic ABI (shared by impl [T] below and by the
// slice expressions handled in codegen).
pub extern fn __slice_len<Z>(s: &[Z]) -> i64;
pub extern fn __slice_get<Z>(s: &[Z], index: i64) -> Option<Z>;
pub extern fn __slice_slice<Z>(s: &[Z], start: i64, end: i64) -> &[Z];
pub extern fn __vec_as_slice<Z>(v: &Vec<Z>) -> &[Z];
pub extern fn __vec_as_mut_slice<Z>(v: &mut Vec<Z>) -> &mut [Z];

impl<T> [T] {
    pub fn len(&self) -> i64 {
        return __slice_len(self);
    }

    pub fn is_empty(&self) -> bool {
        return __slice_len(self) == 0;
    }
}
