fn main() {
    let answer = loop {
        break 42;
    };
    println!("{}", answer);

    let mut count = 0;
    let found = loop {
        count += 1;
        if count > 3 {
            break String::from("big");
        }
    };
    println!("{} {}", count, found);

    let nested = 'outer: loop {
        let mut index = 0;
        loop {
            index += 1;
            if index == 3 {
                break 'outer index * 10;
            }
        }
    };
    println!("{}", nested);

    let mut total = 0;
    for value in vec![1, 2, 3] {
        total += value;
    }
    println!("total {}", total);
}
