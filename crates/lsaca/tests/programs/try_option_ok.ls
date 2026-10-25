// The try operator forwards the Some payload and propagates None.
fn lookup(numbers: Vec<i64>, index: i64) -> Option<i64> {
    let value = numbers.get(index)?;
    Some(value * 2)
}

fn main() {
    let numbers = vec![3, 4, 5];
    match lookup(numbers, 1) {
        Some(value) => println!("some {}", value),
        None => println!("none"),
    }
    match lookup(numbers, 9) {
        Some(value) => println!("some {}", value),
        None => println!("none"),
    }
    println!("done");
}