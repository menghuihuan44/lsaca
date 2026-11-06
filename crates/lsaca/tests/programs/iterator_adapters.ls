// v2.1 iterator adapters and terminal operations.
fn numbers() -> Vec<i64> {
    return vec![1i64, 2, 3, 4, 5, 6];
}

fn main() {
    let pairs: Vec<(i64, i64)> = numbers().iter().zip(vec![10i64, 20, 30, 40].iter()).collect();
    println!("{:?}", pairs);

    let chained: Vec<i64> = vec![1i64, 2].iter().chain(vec![3i64, 4].iter()).collect();
    println!("{:?}", chained);

    let evens: Vec<i64> = numbers()
        .iter()
        .filter_map(|value| if value % 2 == 0 { Option::Some(value * 10) } else { Option::None })
        .collect();
    println!("{:?}", evens);

    let small: Vec<i64> = numbers().iter().take_while(|value| *value < 4).collect();
    println!("{:?}", small);

    let after: Vec<i64> = numbers().iter().skip_while(|value| *value < 3).collect();
    println!("{:?}", after);

    let every: Vec<i64> = numbers().iter().step_by(2).collect();
    println!("{:?}", every);

    let first_two: Vec<i64> = numbers()
        .iter()
        .take(2)
        .inspect(|value| println!("inspect {}", value))
        .collect();
    println!("{:?}", first_two);

    numbers().iter().take(2).for_each(|value| println!("each {}", value));

    println!("{}", numbers().iter().position(|value| value % 4 == 0).unwrap());
    println!("{}", numbers().iter().last().unwrap());
    println!("{}", numbers().iter().nth(2).unwrap());
    println!("{}", numbers().iter().reduce(|left, right| left + right).unwrap());
    println!("{}", numbers().iter().product());
    println!("{}", numbers().iter().sum());

    let defaults = vec![1, 2, 3];
    println!("{}", defaults.iter().sum());
}
