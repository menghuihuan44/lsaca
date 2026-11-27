// rev() walks an iterator backwards and flat_map() expands one item into a
// sequence of items. Both chain with the other lazy adapters.
fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let reversed: Vec<i64> = values.iter().rev().map(|value| *value * 10).collect();
    println!("{:?}", reversed);

    let mut sum: i64 = 0;
    for value in values.iter().rev() {
        sum += *value;
    }
    println!("sum {}", sum);

    let flat: Vec<i64> = values.iter().flat_map(|value| vec![*value, *value * 2]).collect();
    println!("{:?}", flat);

    let mut mixed: i64 = 0;
    for value in values.iter().flat_map(|value| vec![*value, -*value].into_iter()) {
        mixed += value;
    }
    println!("mixed {}", mixed);

    let chained: Vec<i64> = values.iter().rev().flat_map(|value| vec![*value, *value + 100]).collect();
    println!("{:?}", chained);

    let kept: Vec<i64> = values.iter().rev().take(2).map(|value| *value).collect();
    println!("{:?}", kept);
}
