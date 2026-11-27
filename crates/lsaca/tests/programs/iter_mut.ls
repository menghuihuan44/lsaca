// Vec::iter_mut() and "for value in &mut values" borrow the vector mutably and
// yield &mut T, so the loop can change the stored values in place.
fn main() {
    let mut values: Vec<i64> = vec![1, 2, 3, 4];
    for value in values.iter_mut() {
        *value = *value * 10;
    }
    println!("{:?}", values);

    let mut more: Vec<i64> = vec![5, 6, 7];
    for value in &mut more {
        *value += 1;
    }
    println!("{:?}", more);

    let mut words: Vec<String> = vec![String::from("a"), String::from("bb")];
    for word in words.iter_mut() {
        word.push_str("!");
    }
    println!("{:?}", words);
}
