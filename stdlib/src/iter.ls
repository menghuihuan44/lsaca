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