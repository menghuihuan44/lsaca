// Slices iterate through the same borrowing adapters: &[T] yields &T and
// &mut [T] yields &mut T. Vec::as_slice()/as_mut_slice() produce them.
fn main() {
    let values: Vec<i64> = vec![10, 20, 30, 40];
    let slice: &[i64] = values.as_slice();
    let mut sum: i64 = 0;
    for value in slice {
        sum += *value;
    }
    println!("sum {}", sum);

    let doubled: Vec<i64> = slice.iter().map(|value| *value * 2).collect();
    println!("{:?}", doubled);

    let mut mutable: Vec<i64> = vec![1, 2, 3];
    {
        let mutable_slice: &mut [i64] = mutable.as_mut_slice();
        for value in mutable_slice {
            *value += 100;
        }
    }
    let mut total: i64 = 0;
    for value in mutable.as_slice() {
        total += *value;
    }
    println!("{:?} {}", mutable, total);
}
