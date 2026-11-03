// HashMap<K, V>: hash map with i64, u64, isize, usize, bool, char or String keys.
//
// The compiler knows HashMap as a builtin type; this marker struct only claims
// the name for the standard library. The methods below are inherent impls
// backed by extern intrinsics: codegen expands them inline and the legacy
// compiler fallback table no longer participates for these names.
//
// get, get_with and values intentionally stay on the compiler fallback table:
// only the compiler knows which value types can be cloned, and those methods
// report E0277 for values with drop glue (use get_with instead). Keeping them
// there preserves that diagnostic and its wording.
//
// contains_key and remove also stay on the compiler fallback table: in 2.0 their
// key parameter is copied (pinned v1.7 semantics) and a generic stdlib method
// cannot express "K: Clone". A by-reference lookup API is planned for a later
// version; the __map_contains_key/__map_remove intrinsics below are declared
// for that migration and are currently unused.

pub struct HashMap<K, V> {
    internal: (),
}

pub extern fn __map_new<K, V>() -> HashMap<K, V>;
pub extern fn __map_insert<K, V>(table: &mut HashMap<K, V>, key: K, value: V) -> Option<V>;
pub extern fn __map_contains_key<K, V>(table: &HashMap<K, V>, key: K) -> bool;
pub extern fn __map_remove<K, V>(table: &mut HashMap<K, V>, key: K) -> Option<V>;
pub extern fn __map_len<K, V>(table: &HashMap<K, V>) -> i64;
pub extern fn __map_is_empty<K, V>(table: &HashMap<K, V>) -> bool;
pub extern fn __map_clear<K, V>(table: &mut HashMap<K, V>);
pub extern fn __map_keys<K, V>(table: &HashMap<K, V>) -> Vec<K>;

impl<K, V> HashMap<K, V> {
    pub fn new() -> HashMap<K, V> {
        return __map_new::<K, V>();
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        return __map_insert(self, key, value);
    }

    pub fn len(&self) -> i64 {
        return __map_len(self);
    }

    pub fn is_empty(&self) -> bool {
        return __map_is_empty(self);
    }

    pub fn clear(&mut self) {
        __map_clear(self);
    }

    pub fn keys(&self) -> Vec<K> {
        return __map_keys(self);
    }
}
