// User-defined generics: generic structs, generic enums, generic free
// functions, generic inherent impls, turbofish and inference.
struct Pair<T> {
    first: T,
    second: T,
}

enum Maybe<T> {
    Nothing,
    Just(T),
}

impl<T> Pair<T> {
    fn new(first: T, second: T) -> Pair<T> {
        return Pair { first: first, second: second };
    }

    fn second(&self) -> T {
        return self.second;
    }

    fn swap(self) -> Pair<T> {
        return Pair { first: self.second, second: self.first };
    }
}

fn identity<T>(value: T) -> T {
    return value;
}

fn unwrap_or_zero(value: Maybe<i64>) -> i64 {
    match value {
        Maybe::Nothing => 0,
        Maybe::Just(inner) => inner,
    }
}

fn main() {
    let pair = Pair::new(1, 2);
    println!("{} {}", pair.first, pair.second);
    let swapped = pair.swap();
    println!("{} {}", swapped.first, swapped.second);

    let words = Pair::<String>::new(String::from("left"), String::from("right"));
    println!("{} {}", words.first, words.second);

    let explicit = Pair::<i64>::new(3, 4);
    println!("{}", identity(explicit.first));

    let present: Maybe<i64> = Maybe::Just(9);
    let empty: Maybe<i64> = Maybe::Nothing;
    println!("{} {}", unwrap_or_zero(present), unwrap_or_zero(empty));

    let text: Maybe<String> = Maybe::Just(String::from("here"));
    match text {
        Maybe::Nothing => println!("none"),
        Maybe::Just(inner) => println!("{}", inner),
    }
}
