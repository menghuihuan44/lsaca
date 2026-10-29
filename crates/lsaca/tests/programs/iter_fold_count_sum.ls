fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3, 4, 5];
    let total: i64 = numbers.iter().fold(0, |acc, item| acc + item);
    println!("total {}", total);

    let words: Vec<String> = vec![String::from("a"), String::from("b")];
    let joined = words.iter().fold(String::from(""), |acc, item| acc + item);
    println!("joined {}", joined);

    let count = vec![1, 2, 3].iter().count();
    println!("count {}", count);

    let ints: Vec<i64> = vec![1, 2, 3];
    let int_sum: i64 = ints.iter().sum();
    println!("ints {}", int_sum);

    let floats: Vec<f64> = vec![1.25, 2.5];
    let float_sum: f64 = floats.iter().sum();
    println!("floats {}", float_sum);
}
