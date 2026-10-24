fn main() {
    let mut sum = 0;
    for i in 1..=10 {
        sum += i;
    }
    println!("{}", sum);
    let mut n = 0;
    while n < 3 {
        n += 1;
    }
    println!("{}", n);
    let mut count = 0;
    loop {
        count += 1;
        if count == 5 {
            break;
        }
        if count % 2 == 0 {
            continue;
        }
    }
    println!("{}", count);
    let mut product = 1;
    for v in vec![2, 3, 4] {
        product *= v;
    }
    println!("{}", product);
}
