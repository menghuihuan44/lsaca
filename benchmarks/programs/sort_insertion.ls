// sort_insertion: insertion sort over the same LCG sequence as sort_i64 - the
// alternative implementation for the sorting specification (same checksum
// rule: first, last, sum). Insertion sort is O(n^2), so the sizes are small.
// Optional argument: value count (default 1,000, the medium scale).
fn insertion_sort(values: Vec<i64>) -> Vec<i64> {
    let mut data = values;
    let n = data.len();
    let mut i: i64 = 1;
    while i < n {
        let key = data[i];
        let mut j: i64 = i - 1;
        while j >= 0 {
            if data[j] > key {
                data[j + 1] = data[j];
                j = j - 1;
            } else {
                break;
            }
        }
        data[j + 1] = key;
        i = i + 1;
    }
    data
}

fn main() {
    let all = args();
    let n: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 1000 };
    let mut values: Vec<i64> = Vec::new();
    let mut state: i64 = 12345;
    let mut i: i64 = 0;
    while i < n {
        state = (16807 * state) % 2147483647;
        values.push(state);
        i = i + 1;
    }
    let sorted = insertion_sort(values);
    let mut sum: i64 = 0;
    let mut k: i64 = 0;
    while k < n {
        sum = sum + sorted[k];
        k = k + 1;
    }
    println!("checksum={},{},{}", sorted[0], sorted[n - 1], sum);
}