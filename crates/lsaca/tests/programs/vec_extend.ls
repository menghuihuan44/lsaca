fn main() {
    let mut numbers = vec![1, 2, 3];
    numbers.extend(vec![100, 200]);
    println!("{:?}", numbers);
    println!("{}", numbers.len());

    let mut floats = vec![1.5];
    floats.extend(vec![2.5, 3.5]);
    println!("{:?}", floats);

    let mut flags = vec![true];
    flags.extend(vec![false, true]);
    println!("{:?}", flags);

    let mut names: Vec<String> = Vec::new();
    names.extend(vec![String::from("alpha"), String::from("beta")]);
    println!("{:?}", names);
    names.push(String::from("gamma"));
    println!("{:?}", names);
    println!("{}", names.len());
}
