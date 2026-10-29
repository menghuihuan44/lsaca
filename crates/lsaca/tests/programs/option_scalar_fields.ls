// Regression: struct fields of type Option<scalar>, tuple and Box<Option<..>>
// used to generate invalid C.
struct Holder {
    maybe: Option<i64>,
    pair: (i64, bool),
}

fn main() {
    let mut holder = Holder { maybe: Some(5), pair: (1, true) };
    println!("{}", holder.maybe.unwrap());
    println!("{} {}", holder.pair.0, holder.pair.1);
    holder.maybe = None;
    match holder.maybe {
        Some(value) => println!("{}", value),
        None => println!("none"),
    }
    let boxed = Box::new(Some(7));
    println!("{}", boxed.get().unwrap());
    let pair_boxed = Box::new((8, false));
    println!("{} {}", pair_boxed.get().0, pair_boxed.get().1);
}
