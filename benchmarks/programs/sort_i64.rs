// sort_i64: pseudo-random i64 values from a Park-Miller LCG, sorted in place
// with an iterative quicksort (explicit stack, middle pivot, Hoare partition).
// Prints first,last,sum so the checksum covers ordering and the whole array.
// Optional argument: value count (default 200,000, the medium scale).
const SEED: i64 = 12345;

fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
}

fn lcg_next(state: i64) -> i64 {
    (16807 * state) % 2147483647
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n = arg_i64(&args, 0, 200_000) as usize;
    let mut values: Vec<i64> = Vec::with_capacity(n);
    let mut state = SEED;
    for _ in 0..n {
        state = lcg_next(state);
        values.push(state);
    }

    let mut stack: Vec<i64> = Vec::with_capacity(2 * n + 2);
    stack.push(0);
    stack.push((n - 1) as i64);
    while !stack.is_empty() {
        let hi = stack.pop().unwrap();
        let lo = stack.pop().unwrap();
        if lo >= hi {
            continue;
        }
        let mid = lo + (hi - lo) / 2;
        let pivot = values[mid as usize];
        let mut left = lo;
        let mut right = hi;
        while left <= right {
            while values[left as usize] < pivot {
                left += 1;
            }
            while values[right as usize] > pivot {
                right -= 1;
            }
            if left <= right {
                let li = left as usize;
                let ri = right as usize;
                let tmp = values[li];
                values[li] = values[ri];
                values[ri] = tmp;
                left += 1;
                right -= 1;
            }
        }
        if right > lo {
            stack.push(lo);
            stack.push(right);
        }
        if left < hi {
            stack.push(left);
            stack.push(hi);
        }
    }

    let sum: i64 = values.iter().sum();
    println!("checksum={},{},{}", values[0], values[n - 1], sum);
}
