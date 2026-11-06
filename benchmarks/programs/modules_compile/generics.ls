// generics: generic structs, generic enums and generic free functions.
// Part of the modules_compile compile-time workload.
//
// The generic types are instantiated with i64 and String so the module mixes
// monomorphised scalar and owned heap instantiations. Generic code only uses
// operations that are valid for every instantiation: constructing values,
// moving values in and out of containers, and matching on enum payloads.

pub const MODULUS: i64 = 1000000007;

pub enum Maybe<T> {
    Nothing,
    Just(T),
}

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

pub struct Pair<T> {
    first: T,
    second: T,
}

impl<T> Pair<T> {
    pub fn new(first: T, second: T) -> Pair<T> {
        Pair {
            first: first,
            second: second,
        }
    }

    pub fn swap(self) -> Pair<T> {
        Pair {
            first: self.second,
            second: self.first,
        }
    }

    pub fn into_first(self) -> T {
        self.first
    }
}

pub struct Triple<T> {
    pub first: T,
    pub second: T,
    pub third: T,
}

impl<T> Triple<T> {
    pub fn new(first: T, second: T, third: T) -> Triple<T> {
        Triple {
            first: first,
            second: second,
            third: third,
        }
    }

    pub fn rotate(self) -> Triple<T> {
        Triple {
            first: self.second,
            second: self.third,
            third: self.first,
        }
    }

    pub fn into_second(self) -> T {
        self.second
    }
}

pub struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Stack<T> {
        Stack {
            items: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: i64) -> Stack<T> {
        Stack {
            items: Vec::with_capacity(capacity as usize),
        }
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    pub fn len(&self) -> i64 {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.len() == 0
    }
}

pub fn maybe_or_i64(value: Maybe<i64>, fallback: i64) -> i64 {
    match value {
        Maybe::Nothing => fallback,
        Maybe::Just(inner) => inner,
    }
}

pub fn maybe_text_length(value: Maybe<String>) -> i64 {
    match value {
        Maybe::Nothing => 0,
        Maybe::Just(inner) => inner.len() as i64,
    }
}

pub fn maybe_is_just<T>(value: Maybe<T>) -> bool {
    match value {
        Maybe::Nothing => false,
        Maybe::Just(_) => true,
    }
}

pub fn maybe_join(left: Maybe<String>, right: Maybe<String>) -> String {
    let mut joined = String::from("");
    match left {
        Maybe::Nothing => joined.push_str("empty"),
        Maybe::Just(inner) => joined.push_str(inner.as_str()),
    }
    joined.push_str(":");
    match right {
        Maybe::Nothing => joined.push_str("empty"),
        Maybe::Just(inner) => joined.push_str(inner.as_str()),
    }
    joined
}

pub fn either_code(value: Either<String, i64>) -> i64 {
    match value {
        Either::Left(text) => text.len() as i64,
        Either::Right(number) => number,
    }
}

pub fn either_describe(value: Either<i64, String>) -> String {
    match value {
        Either::Left(number) => format!("number {}", number),
        Either::Right(text) => format!("text {}", text),
    }
}

pub fn identity<T>(value: T) -> T {
    value
}

pub fn choose<T>(flag: bool, left: T, right: T) -> T {
    if flag {
        left
    } else {
        right
    }
}

pub fn swap_pair<T>(pair: Pair<T>) -> Pair<T> {
    pair.swap()
}

pub fn first_of_pair<T>(pair: Pair<T>) -> T {
    pair.into_first()
}

pub fn rotate_triple<T>(triple: Triple<T>) -> Triple<T> {
    triple.rotate()
}

pub fn trailing_of_pair(pair: Pair<i64>) -> i64 {
    let second = pair.second;
    second
}

pub fn stack_i64_sum(limit: i64) -> i64 {
    let mut stack: Stack<i64> = Stack::new();
    let mut value = 1;
    while value <= limit {
        stack.push(value);
        value += 1;
    }
    let mut total = 0;
    let mut continue_loop = true;
    while continue_loop {
        match stack.pop() {
            Some(item) => total += item,
            None => continue_loop = false,
        }
    }
    total
}

pub fn stack_i64_levels(limit: i64) -> i64 {
    let mut stack: Stack<i64> = Stack::with_capacity(limit);
    let mut value = 0;
    while value < limit {
        stack.push(value * value);
        value += 1;
    }
    let mut total = 0;
    let mut continue_loop = true;
    while continue_loop {
        match stack.pop() {
            Some(item) => {
                if item % 2 == 0 {
                    total += item;
                }
            }
            None => continue_loop = false,
        }
    }
    total
}

pub fn stack_text_total(words: Vec<String>) -> i64 {
    let mut stack: Stack<String> = Stack::new();
    let mut index = 0;
    while index < words.len() {
        stack.push(format!("{}", words[index]));
        index += 1;
    }
    let mut total = 0;
    let mut continue_loop = true;
    while continue_loop {
        match stack.pop() {
            Some(item) => total += item.len(),
            None => continue_loop = false,
        }
    }
    total as i64
}

pub fn drain_length<T>(values: Vec<T>) -> i64 {
    let mut stack: Stack<T> = Stack::new();
    let mut remaining = values;
    let mut continue_loop = true;
    while continue_loop {
        match remaining.pop() {
            Some(item) => stack.push(item),
            None => continue_loop = false,
        }
    }
    let mut count = 0;
    let mut continue_loop = true;
    while continue_loop {
        match stack.pop() {
            Some(item) => {
                let _ = item;
                count += 1;
            }
            None => continue_loop = false,
        }
    }
    count
}

pub fn first_or<T>(values: Vec<T>, fallback: T) -> T {
    let mut stack: Stack<T> = Stack::new();
    let mut remaining = values;
    let mut continue_loop = true;
    while continue_loop {
        match remaining.pop() {
            Some(item) => stack.push(item),
            None => continue_loop = false,
        }
    }
    match stack.pop() {
        Some(item) => item,
        None => fallback,
    }
}

pub fn pair_checksum(pair: Pair<i64>) -> i64 {
    pair.first * 31 + pair.second
}

pub fn pair_text_checksum(pair: Pair<String>) -> i64 {
    (pair.first.len() as i64) * 31 + pair.second.len() as i64
}

pub fn triple_checksum(triple: Triple<i64>) -> i64 {
    let mut total = triple.first;
    total = total * 31 + triple.second;
    total = total * 31 + triple.third;
    total
}

pub fn triple_rotate_checksum(first: i64, second: i64, third: i64) -> i64 {
    let triple = Triple::new(first, second, third);
    let rotated = triple.rotate();
    triple_checksum(rotated)
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;

    acc = (acc * 31 + identity(42)) % MODULUS;

    let pair = Pair::<i64>::new(3, 4);
    let swapped = swap_pair(pair);
    acc = (acc * 31 + pair_checksum(swapped)) % MODULUS;

    let first = first_of_pair(Pair::<i64>::new(7, 9));
    acc = (acc * 31 + first) % MODULUS;

    let words = Pair::<String>::new(String::from("left"), String::from("right"));
    acc = (acc * 31 + pair_text_checksum(words)) % MODULUS;

    acc = (acc * 31 + triple_checksum(Triple::new(1, 2, 3))) % MODULUS;
    acc = (acc * 31 + triple_rotate_checksum(1, 2, 3)) % MODULUS;

    let chosen = choose(true, 11, 22);
    acc = (acc * 31 + chosen) % MODULUS;
    let chosen_text = choose(false, String::from("no"), String::from("yes"));
    acc = (acc * 31 + chosen_text.len() as i64) % MODULUS;

    acc = (acc * 31 + maybe_or_i64(Maybe::Just(8), 0)) % MODULUS;
    acc = (acc * 31 + maybe_or_i64(Maybe::Nothing, 5)) % MODULUS;
    acc = (acc * 31 + maybe_text_length(Maybe::Just(String::from("generic")))) % MODULUS;
    acc = (acc * 31 + maybe_text_length(Maybe::Nothing)) % MODULUS;
    let one: Maybe<i64> = Maybe::Just(1);
    let present: bool = maybe_is_just(one);
    if present {
        acc = (acc * 31 + 1) % MODULUS;
    }
    let joined = maybe_join(
        Maybe::Just(String::from("a")),
        Maybe::Just(String::from("b")),
    );
    acc = (acc * 31 + joined.len() as i64) % MODULUS;

    acc = (acc * 31 + either_code(Either::Left(String::from("xy")))) % MODULUS;
    acc = (acc * 31 + either_code(Either::Right(9))) % MODULUS;
    let described = either_describe(Either::Left(12));
    acc = (acc * 31 + described.len() as i64) % MODULUS;

    acc = (acc * 31 + stack_i64_sum(5)) % MODULUS;
    acc = (acc * 31 + stack_i64_levels(7)) % MODULUS;

    let word_list: Vec<String> = vec![
        String::from("alpha"),
        String::from("beta"),
        String::from("gamma"),
    ];
    acc = (acc * 31 + stack_text_total(word_list)) % MODULUS;

    let numbers: Vec<i64> = vec![4, 8, 15, 16, 23, 42];
    acc = (acc * 31 + drain_length(numbers)) % MODULUS;

    let fallback = String::from("fallback");
    let first_text = first_or(vec![String::from("only")], fallback);
    acc = (acc * 31 + first_text.len() as i64) % MODULUS;

    let first_number = first_or(vec![1, 2, 3], 0);
    acc = (acc * 31 + first_number) % MODULUS;

    let stack: Stack<i64> = Stack::new();
    let empty = stack.is_empty();
    if empty {
        acc = (acc * 31 + 1) % MODULUS;
    }

    acc % MODULUS
}
