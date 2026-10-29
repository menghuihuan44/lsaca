// Iterators are lazy adapter chains implemented in the lsaca prelude itself:
// crates/lsaca/src/prelude.ls defines the Iterator trait and the MapIter,
// FilterIter, TakeIter, ... structs that the compiler sugar calls into.

struct Countdown {
    remaining: i64,
}

impl Iterator<i64> for Countdown {
    fn next(&mut self) -> Option<i64> {
        if self.remaining <= 0 {
            return None;
        }
        let value = self.remaining;
        self.remaining = self.remaining - 1;
        return Some(value);
    }
}

fn main() {
    // .iter() consumes the Vec and the chain is lazy: nothing runs until the
    // for loop pulls values out.
    let numbers: Vec<i64> = vec![1, 2, 3, 4, 5, 6];
    let total = numbers.iter().filter(|x| *x % 2 == 0).sum();
    println!("sum of evens: {}", total);

    // .collect() needs a known target type; here it comes from the annotation.
    let words: Vec<String> = vec![String::from("alpha"), String::from("beta")];
    let lengths: Vec<i64> = words.iter().map(|word| word.len()).collect();
    println!("lengths: {:?}", lengths);

    // Any type implementing Iterator can be used in a for loop.
    let countdown = Countdown { remaining: 3 };
    let mut count: i64 = 0;
    for value in countdown {
        count = count + value;
    }
    println!("countdown sum: {}", count);

    // fold threads an accumulator through the items.
    let joined = vec![String::from("a"), String::from("b")].iter().fold(String::from(""), |acc, item| acc + item);
    println!("joined: {}", joined);
}
