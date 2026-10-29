fn main() {
    let words: Vec<String> = vec![String::from("hello"), String::from("hi"), String::from("world")];
    let lengths: Vec<i64> = words.iter().map(|word| word.len()).collect();
    println!("{:?}", lengths);

    let rows: Vec<Vec<i64>> = vec![vec![1, 2], vec![3], vec![4, 5, 6]];
    let sizes: Vec<i64> = rows.iter().map(|row| row.len()).collect();
    println!("{:?}", sizes);

    let kept: Vec<String> = vec![String::from("hi"), String::from("hello"), String::from("hey")].iter().filter(|word| word.len() > 2).collect();
    println!("{:?}", kept);
}
