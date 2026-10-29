fn is_long(value: &String) -> bool {
    value.len() > 3
}

fn main() {
    let words = vec![String::from("alpha"), String::from("b")];
    for word in words {
        if is_long(&word) {
            let owned = word;
            println!("{}", owned);
        }
    }

    let base = 10;
    let mut total = 0;
    let mut index = 0;
    while index < 2 {
        let borrowed = &base;
        total = total + *borrowed;
        index = index + 1;
    }
    println!("{} {}", total, index);
}
