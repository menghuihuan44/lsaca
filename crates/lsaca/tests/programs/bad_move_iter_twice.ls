// An iterator adapter is a value: the first for loop moves it, so reusing it in
// a second for loop is E0382.
fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3];
    let doubled = numbers.iter().map(|n| n * 2);
    for value in doubled {
        println!("{}", value);
    }
    for value in doubled {
        println!("{}", value);
    }
}
