fn main() {
    let some: Option<i64> = Some(5);
    if let Some(n) = some {
        println!("got {}", n);
    }
    let mut counter = 0;
    let mut next: Option<i64> = Some(1);
    while let Some(value) = next {
        counter += value;
        if counter > 5 {
            next = None;
        } else {
            next = Some(value + 1);
        }
    }
    println!("{}", counter);
}
