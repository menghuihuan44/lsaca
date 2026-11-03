// Ordered and auxiliary collections.
//
// HashSet   insertion ordered hash set, built on HashMap<K, ()> plus Vec<K>.
// BTreeSet  sorted set over a Vec, kept in order with linear-scan comparisons.
// BTreeMap  sorted map over parallel Vec<K>/Vec<V>.
// VecDeque  double ended queue over a Vec.
// BinaryHeap maximum heap over a Vec kept sorted in descending order.
//
// All containers are pure lsaca code. The ordering containers require the key
// type K to support ==, < and > (the language defers generic comparisons to the
// instantiation), so instantiate them with i64, u64, isize, usize or String.
// The clone based methods require K (and V) to be cloneable, which the compiler
// checks when the generic impl is instantiated.

/// An insertion ordered set of unique values, backed by a hash map plus a
/// vector that preserves insertion order.
pub struct HashSet<K> {
    map: HashMap<K, ()>,
    order: Vec<K>,
}

impl<K> HashSet<K> {
    /// Creates an empty set.
    pub fn new() -> HashSet<K> {
        return HashSet { map: HashMap::new(), order: Vec::new() };
    }

    /// Inserts a value and returns true when it was not present yet.
    pub fn insert(&mut self, key: K) -> bool {
        if self.map.contains_key(key.clone()) {
            return false;
        }
        self.map.insert(key.clone(), ());
        self.order.push(key);
        return true;
    }

    /// Returns true when the set contains the value.
    pub fn contains(&self, key: K) -> bool {
        return self.map.contains_key(key);
    }

    /// Removes a value and returns true when it was present.
    pub fn remove(&mut self, key: K) -> bool {
        if self.map.remove(key.clone()).is_some() {
            let entries = self.order.clone();
            let mut kept: Vec<K> = Vec::new();
            for entry in entries {
                if entry != key {
                    kept.push(entry);
                }
            }
            self.order = kept;
            return true;
        }
        return false;
    }

    /// The number of values in the set.
    pub fn len(&self) -> i64 {
        return self.order.len();
    }

    /// Returns true when the set is empty.
    pub fn is_empty(&self) -> bool {
        return self.order.len() == 0;
    }

    /// Removes every value.
    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// The values in insertion order.
    pub fn elements(&self) -> Vec<K> {
        return self.order.clone();
    }

    /// The values in insertion order, the same as elements.
    pub fn to_vec(&self) -> Vec<K> {
        return self.order.clone();
    }

    /// The set of values that occur in this set or in other.
    pub fn union(&self, other: &HashSet<K>) -> HashSet<K> {
        let mut result: HashSet<K> = HashSet::new();
        let left = self.order.clone();
        for key in left {
            result.insert(key);
        }
        let right = other.order.clone();
        for key in right {
            result.insert(key);
        }
        return result;
    }

    /// The set of values that occur in both sets.
    pub fn intersection(&self, other: &HashSet<K>) -> HashSet<K> {
        let mut result: HashSet<K> = HashSet::new();
        let left = self.order.clone();
        for key in left {
            if other.contains(key.clone()) {
                result.insert(key);
            }
        }
        return result;
    }

    /// The set of values in this set that are not in other.
    pub fn difference(&self, other: &HashSet<K>) -> HashSet<K> {
        let mut result: HashSet<K> = HashSet::new();
        let left = self.order.clone();
        for key in left {
            if !other.contains(key.clone()) {
                result.insert(key);
            }
        }
        return result;
    }

    /// Returns true when every value of this set is contained in other.
    pub fn subset(&self, other: &HashSet<K>) -> bool {
        let left = self.order.clone();
        for key in left {
            if !other.contains(key.clone()) {
                return false;
            }
        }
        return true;
    }
}

/// A sorted set of unique values.
pub struct BTreeSet<K> {
    items: Vec<K>,
}

impl<K> BTreeSet<K> {
    /// Creates an empty set.
    pub fn new() -> BTreeSet<K> {
        return BTreeSet { items: Vec::new() };
    }

    /// Inserts a value and returns true when it was not present yet.
    pub fn insert(&mut self, key: K) -> bool {
        let mut index = 0;
        while index < self.items.len() {
            let current = self.items.get(index).unwrap().clone();
            if current == key {
                return false;
            }
            if current > key {
                break;
            }
            index = index + 1;
        }
        self.items.insert(index, key);
        return true;
    }

    /// Returns true when the set contains the value.
    pub fn contains(&self, key: K) -> bool {
        let mut index = 0;
        while index < self.items.len() {
            let current = self.items.get(index).unwrap().clone();
            if current == key {
                return true;
            }
            if current > key {
                return false;
            }
            index = index + 1;
        }
        return false;
    }

    /// Removes a value and returns true when it was present.
    pub fn remove(&mut self, key: K) -> bool {
        let mut found = -1;
        let mut index = 0;
        while index < self.items.len() {
            let current = self.items.get(index).unwrap().clone();
            if current == key {
                found = index;
                break;
            }
            index = index + 1;
        }
        if found < 0 {
            return false;
        }
        self.items.remove(found);
        return true;
    }

    /// The number of values in the set.
    pub fn len(&self) -> i64 {
        return self.items.len();
    }

    /// Returns true when the set is empty.
    pub fn is_empty(&self) -> bool {
        return self.items.len() == 0;
    }

    /// Removes every value.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// The values in ascending order.
    pub fn elements(&self) -> Vec<K> {
        return self.items.clone();
    }

    /// The smallest value, when the set is not empty.
    pub fn first(&self) -> Option<K> {
        if self.items.len() == 0 {
            return None;
        }
        return Some(self.items.get(0).unwrap().clone());
    }

    /// The largest value, when the set is not empty.
    pub fn last(&self) -> Option<K> {
        if self.items.len() == 0 {
            return None;
        }
        return Some(self.items.get(self.items.len() - 1).unwrap().clone());
    }
}

/// A sorted map from unique keys to values.
pub struct BTreeMap<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K, V> BTreeMap<K, V> {
    /// Creates an empty map.
    pub fn new() -> BTreeMap<K, V> {
        return BTreeMap { keys: Vec::new(), values: Vec::new() };
    }

    /// Finds the insertion position of key and whether it is already present.
    fn find(&self, key: K) -> (i64, bool) {
        let mut index = 0;
        while index < self.keys.len() {
            let current = self.keys.get(index).unwrap().clone();
            if current == key {
                return (index, true);
            }
            if current > key {
                return (index, false);
            }
            index = index + 1;
        }
        return (index, false);
    }

    /// Inserts a value, returning the previous value when the key was present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let position = self.find(key.clone());
        let index = position.0;
        if position.1 {
            let old = self.values.remove(index);
            self.values.insert(index, value);
            return Some(old);
        }
        self.keys.insert(index, key);
        self.values.insert(index, value);
        return None;
    }

    /// The value stored for the key, when present.
    pub fn get(&self, key: K) -> Option<V> {
        let position = self.find(key);
        if position.1 {
            return Some(self.values.get(position.0).unwrap().clone());
        }
        return None;
    }

    /// Returns true when the map contains the key.
    pub fn contains_key(&self, key: K) -> bool {
        let position = self.find(key);
        return position.1;
    }

    /// Removes the value stored for the key, when present.
    pub fn remove(&mut self, key: K) -> Option<V> {
        let position = self.find(key);
        if position.1 {
            let index = position.0;
            self.keys.remove(index);
            return Some(self.values.remove(index));
        }
        return None;
    }

    /// The number of entries in the map.
    pub fn len(&self) -> i64 {
        return self.keys.len();
    }

    /// Returns true when the map is empty.
    pub fn is_empty(&self) -> bool {
        return self.keys.len() == 0;
    }

    /// Removes every entry.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.values.clear();
    }

    /// The keys in ascending order.
    pub fn keys(&self) -> Vec<K> {
        return self.keys.clone();
    }

    /// The values ordered by their keys.
    pub fn values(&self) -> Vec<V> {
        return self.values.clone();
    }
}

/// A double ended queue.
pub struct VecDeque<T> {
    items: Vec<T>,
}

impl<T> VecDeque<T> {
    /// Creates an empty queue.
    pub fn new() -> VecDeque<T> {
        return VecDeque { items: Vec::new() };
    }

    /// Appends a value at the back.
    pub fn push_back(&mut self, value: T) {
        self.items.push(value);
    }

    /// Inserts a value at the front.
    pub fn push_front(&mut self, value: T) {
        self.items.insert(0, value);
    }

    /// Removes and returns the front value, when present.
    pub fn pop_front(&mut self) -> Option<T> {
        if self.items.len() == 0 {
            return None;
        }
        return Some(self.items.remove(0));
    }

    /// Removes and returns the back value, when present.
    pub fn pop_back(&mut self) -> Option<T> {
        if self.items.len() == 0 {
            return None;
        }
        return self.items.pop();
    }

    /// Returns true when the queue is empty.
    pub fn is_empty(&self) -> bool {
        return self.items.len() == 0;
    }

    /// The number of values in the queue.
    pub fn len(&self) -> i64 {
        return self.items.len();
    }

    /// Removes every value.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// The value at the given position counted from the front.
    pub fn get(&self, index: i64) -> Option<T> {
        if index < 0 || index >= self.items.len() {
            return None;
        }
        return Some(self.items.get(index).unwrap().clone());
    }

    /// The values ordered from front to back.
    pub fn elements(&self) -> Vec<T> {
        return self.items.clone();
    }
}

/// A maximum heap.
pub struct BinaryHeap<T> {
    items: Vec<T>,
}

impl<T> BinaryHeap<T> {
    /// Creates an empty heap.
    pub fn new() -> BinaryHeap<T> {
        return BinaryHeap { items: Vec::new() };
    }

    /// The number of values in the heap.
    pub fn len(&self) -> i64 {
        return self.items.len();
    }

    /// Returns true when the heap is empty.
    pub fn is_empty(&self) -> bool {
        return self.items.len() == 0;
    }

    /// Removes every value.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Inserts a value.
    pub fn push(&mut self, value: T) {
        let mut index = 0;
        while index < self.items.len() {
            let current = self.items.get(index).unwrap().clone();
            if current < value {
                break;
            }
            index = index + 1;
        }
        self.items.insert(index, value);
    }

    /// The largest value, when the heap is not empty.
    pub fn peek(&self) -> Option<T> {
        if self.items.len() == 0 {
            return None;
        }
        return Some(self.items.get(0).unwrap().clone());
    }

    /// Removes and returns the largest value, when the heap is not empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.items.len() == 0 {
            return None;
        }
        return Some(self.items.remove(0));
    }

    /// The values ordered from largest to smallest.
    pub fn elements(&self) -> Vec<T> {
        return self.items.clone();
    }
}
