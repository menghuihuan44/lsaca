// The lsaca standard prelude, written in lsaca itself.
//
// This file is embedded into every program by crates/lsaca/src/prelude.rs and
// checked by the type checker like user code (its items are generic items; the
// checker instantiates only what a program uses). The type checker lowers the
// .iter()/.map()/.filter()/.take()/.skip()/.enumerate()/.collect()/.fold()/
// .count()/.sum()/.any()/.all()/.find() sugar to the __lsaca_it_* functions
// below (DESIGN section 5.3); keep this file in sync with that table.
//
// `Iterator` is the only public name. Everything else is __lsaca_ prefixed so
// user definitions cannot collide with it. The VecIter is reversed once up
// front so pop() yields the original order; the Vec is consumed, so the items
// need not be Copy or Clone. Adapter chains are lazy: each struct stores the
// previous iterator and advances it only in next().

pub trait Iterator<T> {
    fn next(&mut self) -> Option<T>;
}

// Iterator over an owned Vec<T>. `.iter()` on a Vec consumes it (documented
// deviation from Rust) and produces this adapter.

struct __lsaca_VecIter<T> { items: Vec<T> }

fn __lsaca_vec_iter<T>(items: Vec<T>) -> __lsaca_VecIter<T> {
    let mut buffer = items;
    buffer.reverse();
    return __lsaca_VecIter { items: buffer };
}

impl<T> Iterator<T> for __lsaca_VecIter<T> {
    fn next(&mut self) -> Option<T> { return self.items.pop(); }
}

// map(f): applies f to every item as it is pulled.

struct __lsaca_MapIter<T, U, I, F> { inner: I, f: F }

// Adapter constructor for `.map(f)`; f is a closure or function value.

fn __lsaca_it_map<T, U, I: Iterator<T>, F: Fn(T) -> U>(iter: I, f: F) -> __lsaca_MapIter<T, U, I, F> {
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

// filter(pred): yields only the items for which pred(&item) is true. The
// predicate sees a reference, but the matching item is still moved onward.

struct __lsaca_FilterIter<T, I, F> { inner: I, pred: F }

// Adapter constructor for `.filter(pred)`.

fn __lsaca_it_filter<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> __lsaca_FilterIter<T, I, F> {
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

struct __lsaca_TakeIter<T, I> { inner: I, remaining: i64 }

// Adapter constructor for `.take(n)`; n is an i64.

fn __lsaca_it_take<T, I: Iterator<T>>(iter: I, count: i64) -> __lsaca_TakeIter<T, I> {
    return __lsaca_TakeIter { inner: iter, remaining: count };
}

impl<T, I: Iterator<T>> Iterator<T> for __lsaca_TakeIter<T, I> {
    fn next(&mut self) -> Option<T> {
        if self.remaining <= 0 { return None; }
        self.remaining = self.remaining - 1;
        return self.inner.next();
    }
}

// skip(n): drops the first n items, then yields the rest.

struct __lsaca_SkipIter<T, I> { inner: I, remaining: i64 }

// Adapter constructor for `.skip(n)`; n is an i64.

fn __lsaca_it_skip<T, I: Iterator<T>>(iter: I, count: i64) -> __lsaca_SkipIter<T, I> {
    return __lsaca_SkipIter { inner: iter, remaining: count };
}

impl<T, I: Iterator<T>> Iterator<T> for __lsaca_SkipIter<T, I> {
    fn next(&mut self) -> Option<T> {
        loop {
            if self.remaining <= 0 { return self.inner.next(); }
            self.remaining = self.remaining - 1;
            match self.inner.next() {
                Some(_skipped) => { }
                None => { return None; }
            }
        }
    }
}

// enumerate(): pairs every item with a zero-based index.

struct __lsaca_EnumerateIter<T, I> { inner: I, count: i64 }

// Adapter constructor for `.enumerate()`; the item type becomes (i64, T).

fn __lsaca_it_enumerate<T, I: Iterator<T>>(iter: I) -> __lsaca_EnumerateIter<T, I> {
    return __lsaca_EnumerateIter { inner: iter, count: 0 };
}

impl<T, I: Iterator<T>> Iterator<(i64, T)> for __lsaca_EnumerateIter<T, I> {
    fn next(&mut self) -> Option<(i64, T)> {
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

// Terminal operations. Each consumes the iterator and releases it at the end
// of its for loop. `.collect()` needs a Vec<T> target from the expected type.

fn __lsaca_it_collect_vec<T, I: Iterator<T>>(iter: I) -> Vec<T> {
    let mut source = iter;
    let mut result: Vec<T> = Vec::new();
    for item in source { result.push(item); }
    return result;
}

// count(): consumes the iterator and returns how many items it produced.

fn __lsaca_it_count<T, I: Iterator<T>>(iter: I) -> i64 {
    let mut source = iter;
    let mut total: i64 = 0;
    for _item in source { total = total + 1; }
    return total;
}

// fold(init, f): threads an accumulator through f(acc, item) for every item.

fn __lsaca_it_fold<T, U, I: Iterator<T>, F: Fn(U, T) -> U>(iter: I, init: U, f: F) -> U {
    let mut source = iter;
    let mut acc = init;
    for item in source { acc = (f)(acc, item); }
    return acc;
}

// sum() for iterators of i64; the checker picks this one by item type.

fn __lsaca_it_sum_i64<I: Iterator<i64>>(iter: I) -> i64 {
    let mut source = iter;
    let mut total: i64 = 0;
    for item in source { total = total + item; }
    return total;
}

// sum() for iterators of f64; the checker picks this one by item type.

fn __lsaca_it_sum_f64<I: Iterator<f64>>(iter: I) -> f64 {
    let mut source = iter;
    let mut total: f64 = 0.0;
    for item in source { total = total + item; }
    return total;
}

// any(pred): true as soon as one item satisfies pred; consumes the rest only
// up to that point. The iterator is released when the function returns.

fn __lsaca_it_any<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> bool {
    let mut source = iter;
    for item in source { if (pred)(&item) { return true; } }
    return false;
}

// all(pred): false as soon as one item fails pred; true otherwise.

fn __lsaca_it_all<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> bool {
    let mut source = iter;
    for item in source { if !(pred)(&item) { return false; } }
    return true;
}

// find(pred): returns the first matching item, or None.

fn __lsaca_it_find<T, I: Iterator<T>, F: Fn(&T) -> bool>(iter: I, pred: F) -> Option<T> {
    let mut source = iter;
    for item in source { if (pred)(&item) { return Some(item); } }
    return None;
}
