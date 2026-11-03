// Functional check: sum_to(n) agrees with the closed form n * (n + 1) / 2
// for a grid of inputs, including the 0/1 boundaries and the largest scale,
// and inside a repeated cycle of small inputs.
fn sum_to(n: i64) -> i64 {
    let mut total: i64 = 0;
    let mut index: i64 = 0;
    while index <= n {
        total += index;
        index += 1;
    }
    total
}

fn main() {
    let sizes: Vec<i64> = vec![0, 1, 2, 3, 10, 999, 1000, 65536, 1000000];
    let mut position = 0;
    while position < sizes.len() {
        let n = sizes[position];
        assert_eq!(sum_to(n), n * (n + 1) / 2);
        position += 1;
    }
    let mut repetitions = 0;
    while repetitions < 1000 {
        let n = repetitions % 17;
        assert_eq!(sum_to(n), n * (n + 1) / 2);
        repetitions += 1;
    }
    println!("ok loop_sum");
}
