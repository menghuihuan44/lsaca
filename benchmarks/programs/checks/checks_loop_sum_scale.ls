// Scale check: the loop_sum workload's summation driven by the same
// small/medium/large arguments that scripts/bench_checks.py passes. The
// printed checksum folds the loop index deterministically, so the runner can
// recompute the expected line in Python without running lsaca.
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
    let all = args();
    let n: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 1000 };
    let total = sum_to(n);
    assert_eq!(total, n * (n + 1) / 2);
    let mut checksum: i64 = 0;
    let mut index: i64 = 1;
    while index <= n {
        checksum = (checksum * 31 + index) % 1000000007;
        index += 1;
    }
    println!("checksum={}", checksum);
}
