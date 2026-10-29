fn main() {
    let words: Vec<String> = vec![String::from("a"), String::from("b")];
    let total: i64 = words.iter().sum();
    println!("{}", total);
}
