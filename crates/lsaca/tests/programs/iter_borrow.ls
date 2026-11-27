// Vec::iter() borrows the vector and yields shared references. The vector is
// still usable after the loop; "for value in &values" is the same iteration.
fn main() {
    let values: Vec<i64> = vec![4, 8, 15, 16, 23, 42];
    let mut sum: i64 = 0;
    for value in values.iter() {
        sum += *value;
    }
    println!("sum {} len {}", sum, values.len());

    let mut doubled: Vec<i64> = Vec::new();
    for (index, value) in values.iter().enumerate() {
        doubled.push(*value * 2 + index as i64);
    }
    println!("{:?}", doubled);

    let mut large: i64 = 0;
    for value in &values {
        if *value > 10 {
            large += 1;
        }
    }
    println!("large {}", large);
}
