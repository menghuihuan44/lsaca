fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3];
    let doubled: Vec<i64> = numbers.iter().map(|n| n * 2).collect();
    println!("{:?}", doubled);

    let words: Vec<String> = vec![String::from("a"), String::from("b")];
    let shouted = words.iter().map(|word| word + "!").collect::<Vec<String>>();
    println!("{:?}", shouted);

    let evens: Vec<i64> = vec![1, 2, 3, 4, 5, 6].iter().filter(|n| *n % 2 == 0).collect();
    println!("{:?}", evens);
}
