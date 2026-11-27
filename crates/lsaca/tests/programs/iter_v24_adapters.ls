// v2.4 iterator adapters: filter_map, zip, chain, take_while, skip_while,
// position, nth and last. The side-effect prints show that every stage stays
// lazy and that terminal operations stop pulling as soon as they can answer.

fn main() {
    // filter_map is lazy: take(1) stops after the first produced item.
    let filtered: Vec<i64> = vec![1i64, 2, 3, 4]
        .into_iter()
        .filter_map(|value| {
            println!("filter_map {}", value);
            if value % 2 == 0 { Some(value * 10) } else { None }
        })
        .take(1)
        .collect();
    println!("filtered {:?}", filtered);

    // zip stops at the shorter input and only pulls what the consumer asks for.
    let zipped: Vec<(i64, i64)> = vec![1i64, 2, 3]
        .into_iter()
        .map(|value| { println!("left {}", value); value })
        .zip(vec![10i64, 20]
            .into_iter()
            .map(|value| { println!("right {}", value); value }))
        .take(1)
        .collect();
    println!("zipped {:?}", zipped);
    let uneven: Vec<(i64, i64)> = vec![1i64, 2, 3]
        .into_iter()
        .zip(vec![10i64, 20].into_iter())
        .collect();
    println!("uneven {:?}", uneven);

    // chain yields all of the first iterator before the second.
    let chained: Vec<i64> = vec![1i64, 2]
        .into_iter()
        .map(|value| { println!("chain left {}", value); value })
        .chain(vec![7i64, 8]
            .into_iter()
            .map(|value| { println!("chain right {}", value); value }))
        .take(1)
        .collect();
    println!("chained {:?}", chained);
    let full_chain: Vec<i64> = vec![1i64, 2].into_iter().chain(vec![7i64, 8].into_iter()).collect();
    println!("full chain {:?}", full_chain);

    // take_while stops calling its predicate at the first false.
    let prefix: Vec<i64> = vec![1i64, 2, 3, 1]
        .into_iter()
        .take_while(|value| {
            println!("take_while {}", *value);
            *value < 3
        })
        .collect();
    println!("prefix {:?}", prefix);

    // skip_while never calls its predicate again after the first false.
    let suffix: Vec<i64> = vec![1i64, 2, 3, 1]
        .into_iter()
        .skip_while(|value| {
            println!("skip_while {}", *value);
            *value < 3
        })
        .collect();
    println!("suffix {:?}", suffix);

    // position stops pulling after the first match.
    let position = vec![1i64, 2, 3, 4]
        .into_iter()
        .map(|value| { println!("position sees {}", value); value })
        .position(|value| value == 2);
    println!("position {:?}", position);
    println!("position missing {:?}", vec![1i64, 2].into_iter().position(|value| value == 9));

    // nth skips its prefix and returns the following item, like Rust.
    let nth = vec![10i64, 20, 30, 40].into_iter().nth(2);
    println!("nth {:?}", nth);
    println!("nth missing {:?}", vec![10i64].into_iter().nth(5));

    // last returns None for an empty iterator.
    println!("last {:?}", vec![4i64, 5, 6].into_iter().last());
    let no_numbers: Vec<i64> = Vec::new();
    println!("last empty {:?}", no_numbers.into_iter().last());
}
