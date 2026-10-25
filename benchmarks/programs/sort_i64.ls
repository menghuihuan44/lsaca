// sort_i64: 200,000 pseudo-random i64 values from a Park-Miller LCG, sorted in
// place with an iterative quicksort (explicit stack, middle pivot, Hoare
// partition). Prints first,last,sum so the checksum covers ordering and the
// whole array.
fn lcg_next(state: i64) -> i64 {
    (16807 * state) % 2147483647
}

fn main() {
    let n: i64 = 200000;
    let mut values: Vec<i64> = Vec::new();
    let mut state: i64 = 12345;
    let mut i = 0;
    while i < n {
        state = lcg_next(state);
        values.push(state);
        i += 1;
    }

    let mut stack: Vec<i64> = Vec::new();
    stack.push(0);
    stack.push(n - 1);
    while stack.len() > 0 {
        let hi = stack.pop().unwrap();
        let lo = stack.pop().unwrap();
        if lo >= hi {
            continue;
        }
        let mid = lo + (hi - lo) / 2;
        let pivot = values[mid];
        let mut left = lo;
        let mut right = hi;
        while left <= right {
            while values[left] < pivot {
                left += 1;
            }
            while values[right] > pivot {
                right -= 1;
            }
            if left <= right {
                let tmp = values[left];
                values[left] = values[right];
                values[right] = tmp;
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

    let first = values[0];
    let last = values[n - 1];
    let mut sum: i64 = 0;
    for v in values {
        sum += v;
    }
    println!("checksum={},{},{}", first, last, sum);
}
