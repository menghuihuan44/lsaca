// Iterators: the Iterator trait and the hidden adapter machinery.
//
// The type checker lowers the .iter()/.map()/.filter()/.take()/.skip()/
// .enumerate()/.collect()/.fold()/.count()/.sum()/.any()/.all()/.find() method
// sugar to calls of the functions below (DESIGN section 5.3; keep this file in
// sync with crates/lsaca/src/typeck/iterators.rs). The crate root re-exports
// every helper explicitly so the sugar can resolve it from the standard crate
// root.
//
// Everything except Iterator is __lsaca_ prefixed so user definitions cannot
// collide with it. The VecIter is reversed once up front so pop() yields the
// original order; the Vec is consumed, so the items need not be Copy or Clone.
// Adapter chains are lazy: each struct stores the previous iterator and
// advances it only in next().
//
// Since 2.1.0 counts are usize: count() returns usize and enumerate() yields
// (usize, T); take()/skip()/step_by()/nth() take usize arguments.

pub trait Iterator<T> {
    fn next(&mut self) -> Option<T>;
}

// Iterator over an owned Vec<T>. .iter() on a Vec consumes it (documented
// deviation from Rust) and produces this adapter.

pub struct __lsaca_VecIter<T> { items: Vec<T> }

pub fn __lsaca_vec_iter<T>(items: Vec<T>) -> __lsaca_VecIter<T> {
    let mut buffer = items;
    buffer.reverse();
    return __lsaca_VecIter { items: buffer };
}

impl<T> Iterator<T> for __lsaca_VecIter<T> {
    fn next(&mut self) -> Option<T> { return self.items.pop(); }
}


// v2.3: borrowed iteration. .iter() walks a Vec by reference instead of
// consuming it, so the vector stays usable after the loop. .iter_mut() yields
// &mut T and hands out one element at a time without allocating. Both are
// lazy; the index advances only in next(). VecIterMut stores a mutable
// reference, which only the prelude may declare as a field, and it implements
// Drop so the auto-Copy rule for reference-containing structs does not let two
// cursors alias the same &mut Vec.

pub struct __lsaca_VecIterRef<'a, T> { items: &'a Vec<T>, index: usize }

pub fn __lsaca_vec_iter_ref<'a, T>(items: &'a Vec<T>) -> __lsaca_VecIterRef<'a, T> {
    return __lsaca_VecIterRef { items: items, index: 0 };
}

impl<'a, T> Iterator<&'a T> for __lsaca_VecIterRef<'a, T> {
    fn next(&mut self) -> Option<&'a T> {
        if self.index >= self.items.len() { return None; }
        let position = self.index;
        self.index = position + 1;
        return Some(&self.items[position]);
    }
}

pub struct __lsaca_VecIterMut<'a, T> { items: &'a mut Vec<T>, index: usize }

pub fn __lsaca_vec_iter_mut<'a, T>(items: &'a mut Vec<T>) -> __lsaca_VecIterMut<'a, T> {
    return __lsaca_VecIterMut { items: items, index: 0 };
}

impl<'a, T> Drop for __lsaca_VecIterMut<'a, T> {
    fn drop(&mut self) { }
}

impl<'a, T> Iterator<&'a mut T> for __lsaca_VecIterMut<'a, T> {
    fn next(&mut self) -> Option<&'a mut T> {
        if self.index >= self.items.len() { return None; }
        let position = self.index;
        self.index = position + 1;
        return self.items.get_mut(position);
    }
}

// v2.3: iteration over &[T] and &mut [T], including Vec::as_slice().

pub struct __lsaca_SliceIter<'a, T> { items: &'a [T], index: usize }

pub fn __lsaca_slice_iter<'a, T>(items: &'a [T]) -> __lsaca_SliceIter<'a, T> {
    return __lsaca_SliceIter { items: items, index: 0 };
}

impl<'a, T> Iterator<&'a T> for __lsaca_SliceIter<'a, T> {
    fn next(&mut self) -> Option<&'a T> {
        if self.index >= self.items.len() { return None; }
        let position = self.index;
        self.index = position + 1;
        return Some(&self.items[position]);
    }
}

pub struct __lsaca_SliceIterMut<'a, T> { items: &'a mut [T], index: usize }

pub fn __lsaca_slice_iter_mut<'a, T>(items: &'a mut [T]) -> __lsaca_SliceIterMut<'a, T> {
    return __lsaca_SliceIterMut { items: items, index: 0 };
}

impl<'a, T> Drop for __lsaca_SliceIterMut<'a, T> {
    fn drop(&mut self) { }
}

impl<'a, T> Iterator<&'a mut T> for __lsaca_SliceIterMut<'a, T> {
    fn next(&mut self) -> Option<&'a mut T> {
        if self.index >= self.items.len() { return None; }
        let position = self.index;
        self.index = position + 1;
        return self.items.get_mut(position);
    }
}

// v2.3: HashMap iteration. The structs are generic over M so this file (which
// is also the embedded compiler-test prelude) does not need to name HashMap;
// the standard library instantiates M with HashMap<K, V>. The intrinsics walk
// the Swiss table in slot order and return the next occupied slot plus
// references into its key/value arrays; iteration therefore borrows and never
// clones.

pub struct __lsaca_HashMapIter<'a, K, V, M> { map: &'a M, index: usize }

extern fn __map_iter_next_index<M>(map: &M, start: usize) -> Option<usize>;
extern fn __map_key_ref<M, K>(map: &M, index: usize) -> &K;
extern fn __map_value_ref<M, V>(map: &M, index: usize) -> &V;

pub fn __lsaca_hash_map_iter<'a, K, V, M>(map: &'a M) -> __lsaca_HashMapIter<'a, K, V, M> {
    return __lsaca_HashMapIter { map: map, index: 0 };
}

impl<'a, K, V, M> Iterator<(&'a K, &'a V)> for __lsaca_HashMapIter<'a, K, V, M> {
    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        match __map_iter_next_index::<M>(self.map, self.index) {
            Some(index) => {
                self.index = index + 1;
                return Some((__map_key_ref::<M, K>(self.map, index), __map_value_ref::<M, V>(self.map, index)));
            }
            None => { return None; }
        }
    }
}

pub struct __lsaca_HashMapKeysIter<'a, K, M> { map: &'a M, index: usize }

pub fn __lsaca_hash_map_keys<'a, K, M>(map: &'a M) -> __lsaca_HashMapKeysIter<'a, K, M> {
    return __lsaca_HashMapKeysIter { map: map, index: 0 };
}

impl<'a, K, M> Iterator<&'a K> for __lsaca_HashMapKeysIter<'a, K, M> {
    fn next(&mut self) -> Option<&'a K> {
        match __map_iter_next_index::<M>(self.map, self.index) {
            Some(index) => {
                self.index = index + 1;
                return Some(__map_key_ref::<M, K>(self.map, index));
            }
            None => { return None; }
        }
    }
}

pub struct __lsaca_HashMapValuesIter<'a, V, M> { map: &'a M, index: usize }

pub fn __lsaca_hash_map_values<'a, V, M>(map: &'a M) -> __lsaca_HashMapValuesIter<'a, V, M> {
    return __lsaca_HashMapValuesIter { map: map, index: 0 };
}

impl<'a, V, M> Iterator<&'a V> for __lsaca_HashMapValuesIter<'a, V, M> {
    fn next(&mut self) -> Option<&'a V> {
        match __map_iter_next_index::<M>(self.map, self.index) {
            Some(index) => {
                self.index = index + 1;
                return Some(__map_value_ref::<M, V>(self.map, index));
            }
            None => { return None; }
        }
    }
}

// map(f): applies f to every item as it is pulled.

pub struct __lsaca_MapIter<T, U, I, F> { inner: I, f: F }

pub fn __lsaca_it_map<T, U, I: Iterator<T>, F: Fn(T) -> U>(iter: I, f: F) -> __lsaca_MapIter<T, U, I, F> {
    return __lsaca_MapIter { inner: iter, f: f };
}

impl<T, U, I: Iterator<T>, F: Fn(T) -> U> Iterator<U> for __lsaca_MapIter<T, U, I, F> {
    fn next(&mut self) -> Option<U> {
        match self.inner.next() {
            Some(item) => { return Some((self.f)(item)); }
            None => { return None; }
        }
    }
}

// filter(pred): yields only the items for which pred(&item) is true.

pub struct __lsaca_FilterIter<T, I, F> { inner: I, pred: F }

pub fn __lsaca_it_filter<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> __lsaca_FilterIter<T, I, F> {
    return __lsaca_FilterIter { inner: iter, pred: pred };
}

impl<T, I: Iterator<T>, F: Fn(&T) -> bool> Iterator<T> for __lsaca_FilterIter<T, I, F> {
    fn next(&mut self) -> Option<T> {
        loop {
            match self.inner.next() {
                Some(item) => {
                    if (self.pred)(&item) { return Some(item); }
                }
                None => { return None; }
            }
        }
    }
}

// take(n): yields at most n items, then stops.

pub struct __lsaca_TakeIter<T, I> { inner: I, remaining: usize }

pub fn __lsaca_it_take<T, I: Iterator<T>>(iter: I, count: usize) -> __lsaca_TakeIter<T, I> {
    return __lsaca_TakeIter { inner: iter, remaining: count };
}

impl<T, I: Iterator<T>> Iterator<T> for __lsaca_TakeIter<T, I> {
    fn next(&mut self) -> Option<T> {
        if self.remaining == 0 { return None; }
        self.remaining = self.remaining - 1;
        return self.inner.next();
    }
}

// skip(n): drops the first n items, then yields the rest.

pub struct __lsaca_SkipIter<T, I> { inner: I, remaining: usize }

pub fn __lsaca_it_skip<T, I: Iterator<T>>(iter: I, count: usize) -> __lsaca_SkipIter<T, I> {
    return __lsaca_SkipIter { inner: iter, remaining: count };
}

impl<T, I: Iterator<T>> Iterator<T> for __lsaca_SkipIter<T, I> {
    fn next(&mut self) -> Option<T> {
        loop {
            if self.remaining == 0 { return self.inner.next(); }
            self.remaining = self.remaining - 1;
            match self.inner.next() {
                Some(_skipped) => { }
                None => { return None; }
            }
        }
    }
}

// enumerate(): pairs every item with a zero-based usize index.

pub struct __lsaca_EnumerateIter<T, I> { inner: I, count: usize }

pub fn __lsaca_it_enumerate<T, I: Iterator<T>>(iter: I) -> __lsaca_EnumerateIter<T, I> {
    return __lsaca_EnumerateIter { inner: iter, count: 0 };
}

impl<T, I: Iterator<T>> Iterator<(usize, T)> for __lsaca_EnumerateIter<T, I> {
    fn next(&mut self) -> Option<(usize, T)> {
        match self.inner.next() {
            Some(item) => {
                let index = self.count;
                self.count = self.count + 1;
                return Some((index, item));
            }
            None => { return None; }
        }
    }
}

// zip(other): pairs the items of both iterators; stops at the shorter one.

pub struct __lsaca_ZipIter<T, U, I, J> { first: I, second: J, done: bool }

pub fn __lsaca_it_zip<T, U, I: Iterator<T>, J: Iterator<U>>(first: I, second: J) -> __lsaca_ZipIter<T, U, I, J> {
    return __lsaca_ZipIter { first: first, second: second, done: false };
}

impl<T, U, I: Iterator<T>, J: Iterator<U>> Iterator<(T, U)> for __lsaca_ZipIter<T, U, I, J> {
    fn next(&mut self) -> Option<(T, U)> {
        if self.done { return None; }
        let left: Option<T> = self.first.next();
        match left {
            Some(right_value) => {
                let right: Option<U> = self.second.next();
                match right {
                    Some(second_value) => { return Some((right_value, second_value)); }
                    None => { self.done = true; return None; }
                }
            }
            None => { self.done = true; return None; }
        }
    }
}

// chain(other): yields the first iterator's items, then the second's.

pub struct __lsaca_ChainIter<T, I, J> { first: I, second: J, first_done: bool }

pub fn __lsaca_it_chain<T, I: Iterator<T>, J: Iterator<T>>(first: I, second: J) -> __lsaca_ChainIter<T, I, J> {
    return __lsaca_ChainIter { first: first, second: second, first_done: false };
}

impl<T, I: Iterator<T>, J: Iterator<T>> Iterator<T> for __lsaca_ChainIter<T, I, J> {
    fn next(&mut self) -> Option<T> {
        if !self.first_done {
            match self.first.next() {
                Some(item) => { return Some(item); }
                None => { self.first_done = true; }
            }
        }
        return self.second.next();
    }
}

// filter_map(f): applies f and keeps only the Some results.

pub struct __lsaca_FilterMapIter<T, U, I, F> { inner: I, f: F }

pub fn __lsaca_it_filter_map<T, U, I: Iterator<T>, F: Fn(T) -> Option<U>>(iter: I, f: F) -> __lsaca_FilterMapIter<T, U, I, F> {
    return __lsaca_FilterMapIter { inner: iter, f: f };
}

impl<T, U, I: Iterator<T>, F: Fn(T) -> Option<U>> Iterator<U> for __lsaca_FilterMapIter<T, U, I, F> {
    fn next(&mut self) -> Option<U> {
        loop {
            match self.inner.next() {
                Some(item) => {
                    match (self.f)(item) {
                        Some(mapped) => { return Some(mapped); }
                        None => { }
                    }
                }
                None => { return None; }
            }
        }
    }
}

// take_while(pred): yields items while pred(&item) is true, then stops.

pub struct __lsaca_TakeWhileIter<T, I, F> { inner: I, pred: F, done: bool }

pub fn __lsaca_it_take_while<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> __lsaca_TakeWhileIter<T, I, F> {
    return __lsaca_TakeWhileIter { inner: iter, pred: pred, done: false };
}

impl<T, I: Iterator<T>, F: Fn(&T) -> bool> Iterator<T> for __lsaca_TakeWhileIter<T, I, F> {
    fn next(&mut self) -> Option<T> {
        if self.done { return None; }
        match self.inner.next() {
            Some(item) => {
                if (self.pred)(&item) { return Some(item); }
                self.done = true;
                return None;
            }
            None => { self.done = true; return None; }
        }
    }
}

// skip_while(pred): drops items while pred(&item) is true, then yields the rest.

pub struct __lsaca_SkipWhileIter<T, I, F> { inner: I, pred: F, skipping: bool }

pub fn __lsaca_it_skip_while<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> __lsaca_SkipWhileIter<T, I, F> {
    return __lsaca_SkipWhileIter { inner: iter, pred: pred, skipping: true };
}

impl<T, I: Iterator<T>, F: Fn(&T) -> bool> Iterator<T> for __lsaca_SkipWhileIter<T, I, F> {
    fn next(&mut self) -> Option<T> {
        loop {
            match self.inner.next() {
                Some(item) => {
                    if self.skipping {
                        if (self.pred)(&item) { } else {
                            self.skipping = false;
                            return Some(item);
                        }
                    } else {
                        return Some(item);
                    }
                }
                None => { return None; }
            }
        }
    }
}

// step_by(step): yields every step-th item starting with the first.

pub struct __lsaca_StepByIter<T, I> { inner: I, step: usize, first: bool }

pub fn __lsaca_it_step_by<T, I: Iterator<T>>(iter: I, step: usize) -> __lsaca_StepByIter<T, I> {
    return __lsaca_StepByIter { inner: iter, step: step, first: true };
}

impl<T, I: Iterator<T>> Iterator<T> for __lsaca_StepByIter<T, I> {
    fn next(&mut self) -> Option<T> {
        if self.step == 0 { return None; }
        if self.first {
            self.first = false;
            return self.inner.next();
        }
        let mut skipped: usize = 0;
        while skipped + 1 < self.step {
            match self.inner.next() {
                Some(_skipped) => { skipped = skipped + 1; }
                None => { return None; }
            }
        }
        return self.inner.next();
    }
}

// inspect(f): calls f with a reference to every item and passes it through.

pub struct __lsaca_InspectIter<T, I, F> { inner: I, f: F }

pub fn __lsaca_it_inspect<T, I: Iterator<T>, F: Fn(&T)>(iter: I, f: F) -> __lsaca_InspectIter<T, I, F> {
    return __lsaca_InspectIter { inner: iter, f: f };
}

impl<T, I: Iterator<T>, F: Fn(&T)> Iterator<T> for __lsaca_InspectIter<T, I, F> {
    fn next(&mut self) -> Option<T> {
        match self.inner.next() {
            Some(item) => {
                (self.f)(&item);
                return Some(item);
            }
            None => { return None; }
        }
    }
}

// v2.3: rev() buffers the iterator's remaining items on the first pull, then
// yields them back to front. Construction itself stays lazy and no size hint
// is claimed: the underlying iterator only needs Iterator<T>, so rev() works on
// every adapter, including flat_map below.

pub struct __lsaca_RevIter<T, I> { inner: I, buffer: Vec<T>, ready: bool }

pub fn __lsaca_it_rev<T, I: Iterator<T>>(iter: I) -> __lsaca_RevIter<T, I> {
    return __lsaca_RevIter { inner: iter, buffer: Vec::new(), ready: false };
}

impl<T, I: Iterator<T>> Iterator<T> for __lsaca_RevIter<T, I> {
    fn next(&mut self) -> Option<T> {
        if !self.ready {
            self.ready = true;
            loop {
                match self.inner.next() {
                    Some(item) => { self.buffer.push(item); }
                    None => { break; }
                }
            }
        }
        return self.buffer.pop();
    }
}

// v2.3: flat_map(f) where f returns a Vec<U>. Each inner Vec is buffered and
// drained in order before the next outer item is pulled, so laziness is per
// outer item and iteration order is preserved.

pub struct __lsaca_FlatMapVecIter<T, U, I, F> { inner: I, f: F, pending: Vec<U> }

pub fn __lsaca_it_flat_map_vec<T, U, I: Iterator<T>, F: Fn(T) -> Vec<U>>(iter: I, f: F) -> __lsaca_FlatMapVecIter<T, U, I, F> {
    return __lsaca_FlatMapVecIter { inner: iter, f: f, pending: Vec::new() };
}

impl<T, U, I: Iterator<T>, F: Fn(T) -> Vec<U>> Iterator<U> for __lsaca_FlatMapVecIter<T, U, I, F> {
    fn next(&mut self) -> Option<U> {
        loop {
            if !self.pending.is_empty() {
                return self.pending.pop();
            }
            match self.inner.next() {
                Some(item) => {
                    let mut produced: Vec<U> = (self.f)(item);
                    produced.reverse();
                    self.pending = produced;
                }
                None => { return None; }
            }
        }
    }
}

// v2.3: flat_map(f) where f returns another iterator. The item type comes from
// that iterator's Iterator<U> implementation. The produced inner iterator is
// drained into a buffer as soon as the outer item is pulled, so iteration stays
// lazy between outer items and order is preserved.

pub struct __lsaca_FlatMapIter<T, U, R, I, F> { inner: I, f: F, pending: Vec<U> }

pub fn __lsaca_it_flat_map<T, U, R: Iterator<U>, I: Iterator<T>, F: Fn(T) -> R>(iter: I, f: F) -> __lsaca_FlatMapIter<T, U, R, I, F> {
    return __lsaca_FlatMapIter { inner: iter, f: f, pending: Vec::new() };
}

impl<T, U, R: Iterator<U>, I: Iterator<T>, F: Fn(T) -> R> Iterator<U> for __lsaca_FlatMapIter<T, U, R, I, F> {
    fn next(&mut self) -> Option<U> {
        loop {
            if !self.pending.is_empty() {
                return self.pending.pop();
            }
            match self.inner.next() {
                Some(item) => {
                    let mut produced: R = (self.f)(item);
                    loop {
                        match produced.next() {
                            Some(value) => { self.pending.push(value); }
                            None => { break; }
                        }
                    }
                    self.pending.reverse();
                }
                None => { return None; }
            }
        }
    }
}

// Terminal operations. Each consumes the iterator and releases it at the end
// of its for loop. .collect() needs a Vec<T> target from the expected type.

pub fn __lsaca_it_collect_vec<T, I: Iterator<T>>(iter: I) -> Vec<T> {
    let mut source = iter;
    let mut result: Vec<T> = Vec::new();
    for item in source { result.push(item); }
    return result;
}

// count(): consumes the iterator and returns how many items it produced.

pub fn __lsaca_it_count<T, I: Iterator<T>>(iter: I) -> usize {
    let mut source = iter;
    let mut total: usize = 0;
    for _item in source { total = total + 1; }
    return total;
}

// fold(init, f): threads an accumulator through f(acc, item) for every item.

pub fn __lsaca_it_fold<T, U, I: Iterator<T>, F: Fn(U, T) -> U>(iter: I, init: U, f: F) -> U {
    let mut source = iter;
    let mut acc = init;
    for item in source { acc = (f)(acc, item); }
    return acc;
}

// for_each(f): calls f with every item for its side effects.

pub fn __lsaca_it_for_each<T, I: Iterator<T>, F: Fn(T)>(iter: I, f: F) {
    let mut source = iter;
    for item in source { (f)(item); }
}

// position(pred): index of the first matching item, or None.

pub fn __lsaca_it_position<T, I: Iterator<T>, F: Fn(T) -> bool>(iter: I, pred: F) -> Option<usize> {
    let mut source = iter;
    let mut index: usize = 0;
    for item in source {
        if (pred)(item) { return Some(index); }
        index = index + 1;
    }
    return None;
}

// last(): consumes the iterator and returns its final item, or None.

pub fn __lsaca_it_last<T, I: Iterator<T>>(iter: I) -> Option<T> {
    let mut source = iter;
    let mut latest: Option<T> = None;
    for item in source { latest = Some(item); }
    return latest;
}

// nth(n): skips n items and returns the next one, or None.

pub fn __lsaca_it_nth<T, I: Iterator<T>>(iter: I, n: usize) -> Option<T> {
    let mut source = iter;
    let mut consumed: usize = 0;
    while consumed < n {
        match source.next() {
            Some(_skipped) => { consumed = consumed + 1; }
            None => { return None; }
        }
    }
    return source.next();
}

// reduce(f): folds with the first item as the initial accumulator.

pub fn __lsaca_it_reduce<T, I: Iterator<T>, F: Fn(T, T) -> T>(iter: I, f: F) -> Option<T> {
    let mut source = iter;
    let mut acc: Option<T> = None;
    for item in source {
        match acc {
            Some(previous) => { acc = Some((f)(previous, item)); }
            None => { acc = Some(item); }
        }
    }
    return acc;
}

// sum(): the checker picks the helper by the iterator's item type.

pub fn __lsaca_it_sum_i32<I: Iterator<i32>>(iter: I) -> i32 {
    let mut source = iter;
    let mut total: i32 = 0;
    for item in source { total = total + item; }
    return total;
}

pub fn __lsaca_it_sum_i64<I: Iterator<i64>>(iter: I) -> i64 {
    let mut source = iter;
    let mut total: i64 = 0;
    for item in source { total = total + item; }
    return total;
}

pub fn __lsaca_it_sum_usize<I: Iterator<usize>>(iter: I) -> usize {
    let mut source = iter;
    let mut total: usize = 0;
    for item in source { total = total + item; }
    return total;
}

pub fn __lsaca_it_sum_f64<I: Iterator<f64>>(iter: I) -> f64 {
    let mut source = iter;
    let mut total: f64 = 0.0;
    for item in source { total = total + item; }
    return total;
}

// product(): like sum(), with 1 as the initial value.

pub fn __lsaca_it_product_i32<I: Iterator<i32>>(iter: I) -> i32 {
    let mut source = iter;
    let mut total: i32 = 1;
    for item in source { total = total * item; }
    return total;
}

pub fn __lsaca_it_product_i64<I: Iterator<i64>>(iter: I) -> i64 {
    let mut source = iter;
    let mut total: i64 = 1;
    for item in source { total = total * item; }
    return total;
}

pub fn __lsaca_it_product_usize<I: Iterator<usize>>(iter: I) -> usize {
    let mut source = iter;
    let mut total: usize = 1;
    for item in source { total = total * item; }
    return total;
}

pub fn __lsaca_it_product_f64<I: Iterator<f64>>(iter: I) -> f64 {
    let mut source = iter;
    let mut total: f64 = 1.0;
    for item in source { total = total * item; }
    return total;
}

// any(pred): true as soon as one item satisfies pred; consumes the rest only
// up to that point. The iterator is released when the function returns.

pub fn __lsaca_it_any<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> bool {
    let mut source = iter;
    for item in source { if (pred)(&item) { return true; } }
    return false;
}

// all(pred): false as soon as one item fails pred; true otherwise.

pub fn __lsaca_it_all<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> bool {
    let mut source = iter;
    for item in source { if !(pred)(&item) { return false; } }
    return true;
}

// find(pred): returns the first matching item, or None.

pub fn __lsaca_it_find<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> Option<T> {
    let mut source = iter;
    for item in source { if (pred)(&item) { return Some(item); } }
    return None;
}