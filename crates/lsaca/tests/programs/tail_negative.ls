fn compute(limit: i64) -> i64 {
    let mut index = 0;
    while index < limit {
        if index == 2 {
            return index;
        }
        index += 1;
    }
    -1
}

fn main() {
    println!("{}", compute(0));
    println!("{}", compute(3));
}
