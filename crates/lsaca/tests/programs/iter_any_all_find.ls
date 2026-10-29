fn main() {
    let numbers: Vec<i64> = vec![5, 10, 15, 20];
    let has_big = numbers.iter().any(|n| *n > 12);
    println!("any {}", has_big);

    let small: Vec<i64> = vec![1, 2, 3];
    let all_small = small.iter().all(|n| *n < 10);
    println!("all {}", all_small);

    let found = vec![3, 6, 9, 12].iter().find(|n| *n > 7);
    match found {
        Some(value) => { println!("found {}", value); }
        None => { println!("not found"); }
    }

    let words: Vec<String> = vec![String::from("hi"), String::from("hello")];
    let long = words.iter().find(|word| word.len() > 2);
    match long {
        Some(word) => { println!("long {}", word); }
        None => { println!("no long word"); }
    }
}
