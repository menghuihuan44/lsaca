fn describe(values: &[i64]) {
    match values {
        [] => println!("empty"),
        [only] => println!("one {}", *only),
        [first, .., last] => println!("ends {} {}", *first, *last),
    }
}

fn main() {
    let empty: Vec<i64> = Vec::new();
    let one = vec![7];
    let many = vec![1, 2, 3, 4];
    describe(&empty);
    describe(&one);
    describe(&many);

    let [a, b, c] = [10, 20, 30];
    println!("sum {}", a + b + c);
    let [first, .., last] = [1, 2, 3, 4];
    println!("{} {}", first, last);
    let [head, ..] = [50, 60, 70];
    println!("head {}", head);

    match [1, 2, 3] {
        [x, .., z] => println!("array {} {}", x, z),
    }

    let point @ (x, y) = (3, 4);
    println!("{:?} {} {}", point, x, y);

    let names = [String::from("alpha"), String::from("beta")];
    let [left, right] = names;
    println!("{} {}", left, right);
    match [String::from("one"), String::from("two")] {
        [only_name, ..] => println!("first {}", only_name),
    }
}
