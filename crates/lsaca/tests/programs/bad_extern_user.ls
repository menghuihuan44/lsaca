extern fn __user_len(value: &Vec<i64>) -> i64;

fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    println!("{}", __user_len(&values));
}
