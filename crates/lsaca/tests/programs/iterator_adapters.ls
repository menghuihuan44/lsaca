// v2.1 iterator adapters and terminal operations.
fn numbers() -> Vec<i64> {
    return vec![1i64, 2, 3, 4, 5, 6];
}

fn main() {
    let pairs: Vec<(i64, i64)> = numbers().into_iter().zip(vec![10i64, 20, 30, 40].into_iter()).collect();
    println!("{:?}", pairs);

    let chained: Vec<i64> = vec![1i64, 2].into_iter().chain(vec![3i64, 4].into_iter()).collect();
    println!("{:?}", chained);

    let evens: Vec<i64> = numbers()
        .into_iter()
        .filter_map(|value| if value % 2 == 0 { Option::Some(value * 10) } else { Option::None })
        .collect();
    println!("{:?}", evens);

    let small: Vec<i64> = numbers().into_iter().take_while(|value| *value < 4).collect();
    println!("{:?}", small);

    let after: Vec<i64> = numbers().into_iter().skip_while(|value| *value < 3).collect();
    println!("{:?}", after);

    let every: Vec<i64> = numbers().into_iter().step_by(2).collect();
    println!("{:?}", every);

    let first_two: Vec<i64> = numbers()
        .into_iter()
        .take(2)
        .inspect(|value| println!("inspect {}", value))
        .collect();
    println!("{:?}", first_two);

    numbers().into_iter().take(2).for_each(|value| println!("each {}", value));

    println!("{}", numbers().into_iter().position(|value| value % 4 == 0).unwrap());
    println!("{}", numbers().into_iter().last().unwrap());
    println!("{}", numbers().into_iter().nth(2).unwrap());
    println!("{}", numbers().into_iter().reduce(|left, right| left + right).unwrap());
    println!("{}", numbers().into_iter().product());
    println!("{}", numbers().into_iter().sum());

    let defaults = vec![1, 2, 3];
    println!("{}", defaults.into_iter().sum());
}
