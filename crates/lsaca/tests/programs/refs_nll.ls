struct Counter {
    count: i64,
}

fn main() {
    let mut value = 1;
    let first = &value;
    let seen = *first;
    let second = &mut value;
    *second = seen + 1;
    println!("{}", value);

    let mut counter = Counter { count: 0 };
    let snapshot = &counter;
    let before = (*snapshot).count;
    let writer = &mut counter;
    (*writer).count = before + 5;
    println!("{}", counter.count);

    let mut total = 0;
    let mut index = 0;
    while index < 3 {
        let borrowed = &counter;
        total = total + borrowed.count;
        index = index + 1;
    }
    println!("{} {}", total, index);
}
