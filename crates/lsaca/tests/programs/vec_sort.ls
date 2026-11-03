// Vector sorting through the stdlib `Vec::sort` method.

fn main() {
    let mut numbers: Vec<i64> = Vec::new();
    numbers.push(5);
    numbers.push(3);
    numbers.push(9);
    numbers.push(1);
    numbers.push(4);
    numbers.sort();
    println!("{:?}", numbers);

    let mut names: Vec<String> = Vec::new();
    names.push(String::from("pear"));
    names.push(String::from("apple"));
    names.push(String::from("fig"));
    names.sort();
    println!("{:?}", names);

    let mut weights: Vec<f64> = Vec::new();
    weights.push(2.5);
    weights.push(0.5);
    weights.push(1.25);
    weights.sort();
    println!("{:?}", weights);

    let mut empty: Vec<i64> = Vec::new();
    empty.sort();
    println!("{}", empty.len());
    let mut single: Vec<i64> = Vec::new();
    single.push(7);
    single.sort();
    println!("{:?}", single);
}
