fn main() {
    let numbers: Vec<i64> = vec![10, 20, 30, 40, 50];
    for value in numbers.iter().skip(1).take(3) {
        println!("{}", value);
    }

    let words: Vec<String> = vec![String::from("a"), String::from("b"), String::from("c")];
    for pair in words.iter().enumerate() {
        let (index, word) = pair;
        println!("{} {}", index, word);
    }
}
