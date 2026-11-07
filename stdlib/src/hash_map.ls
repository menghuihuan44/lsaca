// HashMap<K, V>: hash map with i64, u64, isize, usize, bool, char or String keys.
//
// The compiler knows HashMap as a builtin type; this marker struct only claims
// the name for the standard library. The methods below are inherent impls
// backed by extern intrinsics: codegen expands them inline and the legacy
// compiler fallback table no longer participates for these names.
//
// get and get_mut are borrowed lookups since 2.2: they take `key: &K` and
// return Option<&V> / Option<&mut V>, so neither the key is copied nor the
// value cloned. The returned reference points into the map's storage and keeps
// the map borrow live (v2.2 Phase C borrowed accessors).
//
// get_with and values intentionally stay on the compiler fallback table: only
// the compiler knows which value types can be cloned, and those methods report
// E0277 for values with drop glue (use get_with instead). Keeping them there
// preserves that diagnostic and its wording.
//
// Since 2.1.0 len() returns usize like the other collection lengths.
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

// v2.2 Phase C borrowed-lookup intrinsics: return a reference to the stored
// value, or None when the key is absent. The pointer is valid until the map is
// mutated (insert/remove/clear); the borrow checker ties it to the table borrow.
// The lookup key is passed by value because the return-elision rule only
// attributes the returned reference when the function has exactly one
// reference parameter (lifetime annotations are not part of the language);
// the wrapper clones the caller's &K, which is a COW retain for String keys.
// The intrinsic consumes the key and must release it after the lookup.
pub extern fn __map_get_ref<K, V>(table: &HashMap<K, V>, key: K) -> Option<&V>;
pub extern fn __map_get_mut<K, V>(table: &mut HashMap<K, V>, key: K) -> Option<&mut V>;

impl<K, V> HashMap<K, V> {
    pub fn new() -> HashMap<K, V> {
        return __map_new::<K, V>();
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        return __map_insert(self, key, value);
    }

    pub fn len(&self) -> usize {
        return __map_len(self) as usize;
    }

    pub fn is_empty(&self) -> bool {
        return __map_is_empty(self);
    }

    /// Returns a shared reference to the value stored for `key`, or None when
    /// the key is absent (v2.2 borrowed lookups).
    pub fn get(&self, key: &K) -> Option<&V> {
        let lookup_key: K = key.clone();
        return __map_get_ref(self, lookup_key);
    }

    /// Returns a mutable reference to the value stored for `key`, or None when
    /// the key is absent (v2.2 borrowed lookups).
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let lookup_key: K = key.clone();
        return __map_get_mut(self, lookup_key);
    }

    pub fn clear(&mut self) {
        __map_clear(self);
    }

    pub fn keys(&self) -> Vec<K> {
        return __map_keys(self);
    }
}
